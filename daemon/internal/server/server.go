// Package server is whiterosed's socket: the shell and the Settings app
// connect to it to read and change settings, ask for calendar events, and get
// pushed every change (settings, system state, lock requests) as it happens.
//
// Newline-delimited JSON. Requests carry an id that the reply repeats:
//
//	{"id":1,"op":"get","key":"dock.pinned"}           -> {"reply":1,"value":[...]}
//	{"id":2,"op":"set","key":"quick.doNotDisturb","value":true}
//	{"id":3,"op":"reset","key":"weather.city"}
//	{"id":4,"op":"all"}                                -> {"reply":4,"value":{...}}
//	{"id":5,"op":"calendar_events","from":"2026-10-26","to":"2026-12-07"}
//	{"id":6,"op":"calendars"}                          -> names, colours, no links
//	{"id":7,"op":"add_calendar","name":"Work","url":"https://…","color":"#8FB8F2"}
//	{"id":8,"op":"remove_calendar","index":1}
//
// Failures reply {"reply":id,"error":"…"}. Pushed events:
//
//	{"event":"settings","values":{...}}   on connect
//	{"event":"setting","key":…,"value":…} on every change
//	{"event":"state",…}                   system state (brightness, radios, weather, …)
//	{"event":"lock"} / {"event":"unlock"}  logind's requests
package server

import (
	"bufio"
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"strings"
	"sync"
	"time"

	"whiterose/daemon/internal/calendar"
	"whiterose/daemon/internal/store"
)

type Server struct {
	Store   *store.Store
	Secrets *store.Resolver
	Dirs    calendar.Dirs
	// Called after calendars are added or removed (to fetch them).
	CalendarsChanged func()
	// Extra requests, by op name (battery details, ...), kept out of this
	// package so it doesn't depend on every subsystem.
	Ops map[string]func(Request) (any, error)

	mu      sync.Mutex
	clients map[*client]struct{}
	state   json.RawMessage
}

type client struct {
	conn net.Conn
	out  chan []byte // never closed: senders may still hold the client
	done chan struct{}
	once sync.Once
}

func (c *client) close() {
	c.once.Do(func() {
		close(c.done)
		c.conn.Close()
	})
}

// send queues a line; a client too slow to keep up is dropped (it reconnects
// and gets everything again) rather than holding up the daemon.
func (c *client) send(line []byte) {
	select {
	case c.out <- line:
	default:
		c.close()
	}
}

func line(v any) []byte {
	b, err := json.Marshal(v)
	if err != nil {
		b, _ = json.Marshal(map[string]string{"error": err.Error()})
	}
	return append(b, '\n')
}

// Serve accepts clients until the listener closes, and pushes settings changes.
func (s *Server) Serve(ln net.Listener) error {
	s.mu.Lock()
	if s.clients == nil {
		s.clients = map[*client]struct{}{}
	}
	s.mu.Unlock()
	changes, stop := s.Store.Subscribe()
	defer stop()
	go func() {
		for c := range changes {
			s.Broadcast(map[string]any{"event": "setting", "key": c.Key, "value": c.Value})
		}
	}()
	for {
		conn, err := ln.Accept()
		if err != nil {
			if errors.Is(err, net.ErrClosed) {
				return nil
			}
			return err
		}
		go s.serveClient(conn)
	}
}

// Broadcast sends an event to every client.
func (s *Server) Broadcast(v any) {
	b := line(v)
	s.mu.Lock()
	defer s.mu.Unlock()
	for c := range s.clients {
		c.send(b)
	}
}

// SetState records the system state and pushes it if it changed.
func (s *Server) SetState(v any) {
	b, err := json.Marshal(v)
	if err != nil {
		return
	}
	var m map[string]any
	_ = json.Unmarshal(b, &m)
	if m == nil {
		m = map[string]any{}
	}
	m["event"] = "state"
	b, _ = json.Marshal(m)
	s.mu.Lock()
	same := string(b) == string(s.state)
	s.state = b
	s.mu.Unlock()
	if !same {
		s.Broadcast(json.RawMessage(b))
	}
}

const maxLine = 4 << 20

func (s *Server) serveClient(conn net.Conn) {
	c := &client{conn: conn, out: make(chan []byte, 256), done: make(chan struct{})}
	all, err := s.Store.All()
	if err != nil {
		conn.Close()
		return
	}
	c.out <- line(map[string]any{"event": "settings", "values": all})
	s.mu.Lock()
	if s.state != nil {
		c.out <- append(append([]byte(nil), s.state...), '\n')
	}
	s.clients[c] = struct{}{}
	s.mu.Unlock()
	defer func() {
		s.mu.Lock()
		delete(s.clients, c)
		s.mu.Unlock()
		c.close()
	}()

	go func() {
		for {
			select {
			case <-c.done:
				return
			case b := <-c.out:
				_ = conn.SetWriteDeadline(time.Now().Add(5 * time.Second))
				if _, err := conn.Write(b); err != nil {
					c.close()
					return
				}
			}
		}
	}()

	sc := bufio.NewScanner(conn)
	sc.Buffer(make([]byte, 64*1024), maxLine)
	for sc.Scan() {
		if strings.TrimSpace(sc.Text()) == "" {
			continue
		}
		var req Request
		if err := json.Unmarshal(sc.Bytes(), &req); err != nil {
			c.send(line(map[string]any{"error": "bad request: " + err.Error()}))
			continue
		}
		answer := func() {
			value, err := s.Handle(req)
			reply := map[string]any{"reply": req.ID}
			if err != nil {
				reply["error"] = err.Error()
			} else if value != nil {
				reply["value"] = value
			}
			c.send(line(reply))
		}
		// Extra ops can be slow (a polkit password prompt, a D-Bus call):
		// they answer when done, without holding up this client's other
		// requests. Settings requests stay in order.
		if _, slow := s.Ops[req.Op]; slow {
			go answer()
		} else {
			answer()
		}
	}
}

// Request is one client request.
type Request struct {
	ID    int64           `json:"id"`
	Op    string          `json:"op"`
	Key   string          `json:"key"`
	Value json.RawMessage `json:"value"`
	// calendar_events
	From string `json:"from"`
	To   string `json:"to"`
	// add_calendar / remove_calendar
	Name  string `json:"name"`
	URL   string `json:"url"`
	Path  string `json:"path"`
	Color string `json:"color"`
	Index *int   `json:"index"`
	// set_charge_limit
	On bool `json:"on"`
	// network_profile / network_update
	UUID    string          `json:"uuid"`
	Changes json.RawMessage `json:"changes"`
	// Other ops' arguments (users, ...).
	Args json.RawMessage `json:"args"`
}

// Handle runs one request (also used directly by the CLI and tests).
func (s *Server) Handle(r Request) (any, error) {
	if op, ok := s.Ops[r.Op]; ok {
		return op(r)
	}
	switch r.Op {
	case "get":
		v, ok, err := s.Store.Get(r.Key)
		if err != nil {
			return nil, err
		}
		if !ok {
			return nil, fmt.Errorf("unknown setting %q", r.Key)
		}
		return v, nil
	case "all":
		return s.Store.All()
	case "set":
		if k, ok := store.Known[r.Key]; ok && k.ReadOnly {
			return nil, fmt.Errorf("%s is changed with its own requests", r.Key)
		}
		if len(r.Value) == 0 {
			return nil, errors.New("set needs a value")
		}
		return nil, s.Store.Set(r.Key, r.Value)
	case "reset":
		if k, ok := store.Known[r.Key]; ok && k.ReadOnly {
			return nil, fmt.Errorf("%s is changed with its own requests", r.Key)
		}
		return nil, s.Store.Reset(r.Key)
	case "calendars":
		return s.calendarList()
	case "add_calendar":
		return nil, s.addCalendar(r)
	case "remove_calendar":
		return nil, s.removeCalendar(r)
	case "calendar_events":
		from, err1 := time.ParseInLocation("2006-01-02", r.From, time.Local)
		to, err2 := time.ParseInLocation("2006-01-02", r.To, time.Local)
		if err1 != nil || err2 != nil || !to.After(from) || to.Sub(from) > 400*24*time.Hour {
			return nil, errors.New("from and to must be dates (YYYY-MM-DD), to after from, at most a year apart")
		}
		return calendar.Events(s.Dirs, s.CalendarSources(), from, to, time.Local), nil
	}
	return nil, fmt.Errorf("unknown op %q", r.Op)
}

// CalendarSources resolves the configured calendars' links from the secret
// store and adds dropped-in files. A calendar whose link can't be read
// (the keyring is locked) is still listed, without its link: its last
// download shows, and it isn't fetched until the link can be read.
func (s *Server) CalendarSources() []calendar.Source {
	srcs, _ := s.calendarSources()
	return srcs
}

// calendarSources is CalendarSources, and the calendars whose links
// couldn't be read and why.
func (s *Server) calendarSources() ([]calendar.Source, map[string]error) {
	var stored []store.CalendarSource
	_ = s.Store.GetInto("calendar.sources", &stored)
	var out []calendar.Source
	unread := map[string]error{}
	for _, c := range stored {
		src := calendar.Source{Name: c.Name, Color: c.Color, Path: c.Path, ID: c.Secret}
		if c.Secret != "" {
			url, err := s.Secrets.Get(c.Secret)
			if err != nil {
				unread[c.Name] = err
			}
			src.URL = url
		}
		out = append(out, src)
	}
	return calendar.Sources(s.Dirs, out), unread
}

// CalendarLinksUnread is why each calendar whose link can't be read now
// can't (logged when calendars are fetched).
func (s *Server) CalendarLinksUnread() map[string]error {
	_, unread := s.calendarSources()
	return unread
}

// calendarView is a calendar as clients see it: never the link itself.
type calendarView struct {
	Name    string `json:"name"`
	Color   string `json:"color"`
	Path    string `json:"path,omitempty"`
	HasLink bool   `json:"has_link"`
	// The link is in the keyring, which is locked: it can't be fetched.
	Locked bool `json:"locked,omitempty"`
}

func (s *Server) calendarList() ([]calendarView, error) {
	var stored []store.CalendarSource
	if err := s.Store.GetInto("calendar.sources", &stored); err != nil {
		return nil, err
	}
	out := []calendarView{}
	for i, c := range stored {
		color := c.Color
		if color == "" {
			color = calendar.Palette[i%len(calendar.Palette)]
		}
		v := calendarView{Name: c.Name, Color: color, Path: c.Path, HasLink: c.Secret != ""}
		if v.HasLink {
			_, err := s.Secrets.Get(c.Secret)
			v.Locked = errors.Is(err, store.ErrLocked)
		}
		out = append(out, v)
	}
	return out, nil
}

func (s *Server) addCalendar(r Request) error {
	if strings.TrimSpace(r.Name) == "" || (r.URL == "") == (r.Path == "") {
		return errors.New("a calendar needs a name and either a link or a file")
	}
	if r.URL != "" && !strings.HasPrefix(r.URL, "https://") && !strings.HasPrefix(r.URL, "http://") && !strings.HasPrefix(r.URL, "webcal://") {
		return errors.New("calendar links must be https://, http:// or webcal://")
	}
	var stored []store.CalendarSource
	_ = s.Store.GetInto("calendar.sources", &stored)
	src := store.CalendarSource{Name: strings.TrimSpace(r.Name), Color: r.Color, Path: r.Path}
	if r.URL != "" {
		src.Secret = store.NewSecretID()
		if _, err := s.Secrets.Set(src.Secret, r.URL); err != nil {
			return fmt.Errorf("storing the link: %w", err)
		}
	}
	if err := s.Store.SetValue("calendar.sources", append(stored, src)); err != nil {
		if src.Secret != "" {
			s.Secrets.Delete(src.Secret)
		}
		return err
	}
	if s.CalendarsChanged != nil {
		go s.CalendarsChanged()
	}
	return nil
}

func (s *Server) removeCalendar(r Request) error {
	var stored []store.CalendarSource
	_ = s.Store.GetInto("calendar.sources", &stored)
	if r.Index == nil || *r.Index < 0 || *r.Index >= len(stored) {
		return errors.New("no such calendar")
	}
	gone := stored[*r.Index]
	stored = append(stored[:*r.Index], stored[*r.Index+1:]...)
	if err := s.Store.SetValue("calendar.sources", stored); err != nil {
		return err
	}
	if gone.Secret != "" {
		s.Secrets.Delete(gone.Secret)
	}
	if s.CalendarsChanged != nil {
		go s.CalendarsChanged()
	}
	return nil
}
