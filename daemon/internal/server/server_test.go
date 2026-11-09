package server

import (
	"bufio"
	"encoding/json"
	"net"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"whiterose/daemon/internal/calendar"
	"whiterose/daemon/internal/store"
)

type conn struct {
	t   *testing.T
	c   net.Conn
	r   *bufio.Scanner
	id  int64
	raw []string // every line received, for leak checks
}

func dial(t *testing.T, path string) *conn {
	c, err := net.Dial("unix", path)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { c.Close() })
	sc := bufio.NewScanner(c)
	sc.Buffer(make([]byte, 1<<20), 1<<22)
	return &conn{t: t, c: c, r: sc}
}

func (c *conn) next() map[string]any {
	c.t.Helper()
	_ = c.c.SetReadDeadline(time.Now().Add(3 * time.Second))
	if !c.r.Scan() {
		c.t.Fatalf("connection ended: %v", c.r.Err())
	}
	c.raw = append(c.raw, c.r.Text())
	var m map[string]any
	if err := json.Unmarshal(c.r.Bytes(), &m); err != nil {
		c.t.Fatal(err)
	}
	return m
}

// until reads until a message matches.
func (c *conn) until(match func(map[string]any) bool) map[string]any {
	c.t.Helper()
	for {
		if m := c.next(); match(m) {
			return m
		}
	}
}

func (c *conn) do(req map[string]any) map[string]any {
	c.t.Helper()
	c.id++
	req["id"] = c.id
	b, _ := json.Marshal(req)
	if _, err := c.c.Write(append(b, '\n')); err != nil {
		c.t.Fatal(err)
	}
	id := float64(c.id)
	return c.until(func(m map[string]any) bool { return m["reply"] == id })
}

func setup(t *testing.T) (*Server, string, calendar.Dirs) {
	root := t.TempDir()
	st, err := store.Open(filepath.Join(root, "settings.db"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { st.Close() })
	d := calendar.Dirs{Config: filepath.Join(root, "c"), Data: filepath.Join(root, "d"), Cache: filepath.Join(root, "k"), Home: root}
	s := &Server{Store: st, Secrets: store.Resolver{Fallback: st.Fallback()}, Dirs: d}
	sock := filepath.Join(root, "d.sock")
	ln, err := net.Listen("unix", sock)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { ln.Close() })
	go s.Serve(ln)
	return s, sock, d
}

func TestSettingsArePushedToEveryClient(t *testing.T) {
	_, sock, _ := setup(t)
	a, b := dial(t, sock), dial(t, sock)
	for _, c := range []*conn{a, b} {
		first := c.next()
		vals, _ := first["values"].(map[string]any)
		if first["event"] != "settings" || vals["quick.doNotDisturb"] != false {
			t.Fatalf("first message: %v", first)
		}
	}
	if r := a.do(map[string]any{"op": "set", "key": "quick.doNotDisturb", "value": true}); r["error"] != nil {
		t.Fatalf("set: %v", r)
	}
	got := b.until(func(m map[string]any) bool { return m["event"] == "setting" })
	if got["key"] != "quick.doNotDisturb" || got["value"] != true {
		t.Fatalf("pushed: %v", got)
	}
	if r := b.do(map[string]any{"op": "get", "key": "quick.doNotDisturb"}); r["value"] != true {
		t.Fatalf("get: %v", r)
	}
	// Bad requests get an error and the connection carries on.
	for _, req := range []map[string]any{
		{"op": "set", "key": "quick.doNotDisturb", "value": "yes"},
		{"op": "set", "key": "calendar.sources", "value": []any{}},
		{"op": "set", "key": "no.such.thing", "value": 1},
		{"op": "fly"},
	} {
		if r := a.do(req); r["error"] == nil {
			t.Errorf("%v accepted", req)
		}
	}
	_, _ = a.c.Write([]byte("not json\n"))
	a.until(func(m map[string]any) bool { return m["error"] != nil && m["reply"] == nil })
	if r := a.do(map[string]any{"op": "reset", "key": "quick.doNotDisturb"}); r["error"] != nil {
		t.Fatal(r)
	}
}

func TestCalendarLinksNeverLeaveTheSecretStore(t *testing.T) {
	s, sock, d := setup(t)
	refreshed := make(chan struct{}, 4)
	s.CalendarsChanged = func() { refreshed <- struct{}{} }
	ics := "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:1\r\nSUMMARY:Gym\r\nDTSTART:20261109T183000\r\nDTEND:20261109T193000\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
	if err := os.WriteFile(filepath.Join(d.Home, "work.ics"), []byte(ics), 0o644); err != nil {
		t.Fatal(err)
	}
	c := dial(t, sock)
	c.next()
	if r := c.do(map[string]any{"op": "add_calendar", "name": "Personal", "url": "https://cal.example/private-abc123/basic.ics"}); r["error"] != nil {
		t.Fatal(r)
	}
	if r := c.do(map[string]any{"op": "add_calendar", "name": "Work", "path": "~/work.ics", "color": "#123456"}); r["error"] != nil {
		t.Fatal(r)
	}
	for _, bad := range []map[string]any{
		{"op": "add_calendar", "name": "X", "url": "ftp://x"},
		{"op": "add_calendar", "name": "", "path": "/x.ics"},
		{"op": "add_calendar", "name": "Both", "url": "https://a", "path": "/b"},
	} {
		if r := c.do(bad); r["error"] == nil {
			t.Errorf("%v accepted", bad)
		}
	}
	<-refreshed
	list := c.do(map[string]any{"op": "calendars"})["value"].([]any)
	if len(list) != 2 || list[0].(map[string]any)["has_link"] != true || list[1].(map[string]any)["color"] != "#123456" {
		t.Fatalf("calendars: %v", list)
	}
	ev := c.do(map[string]any{"op": "calendar_events", "from": "2026-11-09", "to": "2026-11-10"})["value"].([]any)
	if len(ev) != 1 || ev[0].(map[string]any)["title"] != "Gym" {
		t.Fatalf("events: %v", ev)
	}
	if r := c.do(map[string]any{"op": "calendar_events", "from": "2026-11-10", "to": "2026-11-09"}); r["error"] == nil {
		t.Fatal("backwards range accepted")
	}
	all, _ := s.Store.All()
	for k, v := range all {
		if strings.Contains(string(v), "private-abc123") {
			t.Fatalf("link stored in setting %s", k)
		}
	}
	for _, l := range c.raw {
		if strings.Contains(l, "private-abc123") {
			t.Fatalf("link sent to a client: %s", l)
		}
	}
	if srcs := s.CalendarSources(); len(srcs) != 2 || srcs[0].URL != "https://cal.example/private-abc123/basic.ics" {
		t.Fatalf("the daemon itself resolves the link: %+v", srcs)
	}
	// Removing the calendar removes its link too.
	var stored []store.CalendarSource
	_ = s.Store.GetInto("calendar.sources", &stored)
	secret := stored[0].Secret
	if r := c.do(map[string]any{"op": "remove_calendar", "index": 0}); r["error"] != nil {
		t.Fatal(r)
	}
	if _, err := s.Secrets.Get(secret); err == nil {
		t.Fatal("removed calendar's link still stored")
	}
	if r := c.do(map[string]any{"op": "remove_calendar", "index": 5}); r["error"] == nil {
		t.Fatal("bad index accepted")
	}
}

func TestStateIsPushedOnlyWhenItChanges(t *testing.T) {
	s, sock, _ := setup(t)
	a := dial(t, sock)
	a.next()
	s.SetState(map[string]any{"brightness": 50})
	s.SetState(map[string]any{"brightness": 50})
	s.SetState(map[string]any{"brightness": 60})
	first := a.until(func(m map[string]any) bool { return m["event"] == "state" })
	second := a.until(func(m map[string]any) bool { return m["event"] == "state" })
	if first["brightness"] != float64(50) || second["brightness"] != float64(60) {
		t.Fatalf("got %v then %v (a repeat must not be sent)", first, second)
	}
	// A new client gets the latest state straight away.
	b := dial(t, sock)
	b.next()
	if m := b.next(); m["event"] != "state" || m["brightness"] != float64(60) {
		t.Fatalf("new client: %v", m)
	}
}
