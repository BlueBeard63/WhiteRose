package main

import (
	"bufio"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"os/signal"
	"path/filepath"
	"strings"
	"sync"
	"syscall"
	"time"
	"whiterose/daemon/internal/accounts"
	"whiterose/daemon/internal/audio"

	"github.com/godbus/dbus/v5"

	"whiterose/daemon/internal/backlight"
	"whiterose/daemon/internal/calendar"
	"whiterose/daemon/internal/migrate"
	"whiterose/daemon/internal/netconf"
	"whiterose/daemon/internal/power"
	"whiterose/daemon/internal/rfkill"
	"whiterose/daemon/internal/server"
	"whiterose/daemon/internal/session"
	"whiterose/daemon/internal/store"
	"whiterose/daemon/internal/uevent"
	"whiterose/daemon/internal/weather"
)

var httpClient = &http.Client{Timeout: 20 * time.Second}

// daemonSocket is where whiterosed listens: $WHITEROSE_DAEMON_SOCKET (set by
// the compositor for its session) or $XDG_RUNTIME_DIR/whiterosed.sock.
func daemonSocket() string {
	if p := os.Getenv("WHITEROSE_DAEMON_SOCKET"); p != "" {
		return p
	}
	dir := os.Getenv("XDG_RUNTIME_DIR")
	if dir == "" {
		dir = os.TempDir()
	}
	return filepath.Join(dir, "whiterosed.sock")
}

func weatherConfig(st *store.Store) weather.Config {
	var c weather.Config
	_ = st.GetInto("weather.city", &c.City)
	_ = st.GetInto("weather.latitude", &c.Latitude)
	_ = st.GetInto("weather.longitude", &c.Longitude)
	_ = st.GetInto("weather.units", &c.Units)
	return c
}

// weatherView is the cached reading for the place now chosen, or nil.
func weatherView(st *store.Store, cacheDir string) *weather.Now {
	cfg := weatherConfig(st)
	if !cfg.Configured() {
		return nil
	}
	n, ok := weather.Cached(cacheDir)
	if !ok || n.City != cfg.City {
		return nil
	}
	return &n
}

// localSources: the calendars that are files (for change detection, which
// mustn't need the keyring on every brightness tick).
func localSources(st *store.Store, d calendar.Dirs) []calendar.Source {
	var stored []store.CalendarSource
	_ = st.GetInto("calendar.sources", &stored)
	var out []calendar.Source
	for _, c := range stored {
		if c.Path != "" {
			out = append(out, calendar.Source{Name: c.Name, Path: c.Path})
		}
	}
	return calendar.Sources(d, out)
}

// cmdServe runs the daemon: the settings database and its socket, the
// system state (brightness, radios), calendar and weather refreshes, and
// logind's lock requests. One per session.
func cmdServe(_ []string, stdout, stderr io.Writer) int {
	sock := daemonSocket()
	if c, err := net.Dial("unix", sock); err == nil {
		c.Close()
		fmt.Fprintln(stderr, "whiterosed: already running at", sock)
		return 0
	}
	_ = os.Remove(sock) // a stale socket from a crash

	st, err := store.Open(store.DefaultPath())
	if err != nil {
		fmt.Fprintln(stderr, "whiterosed:", err)
		return 1
	}
	defer st.Close()

	secrets := store.Resolver{Fallback: st.Fallback()}
	if kr, err := store.Keyring(); err == nil {
		secrets.Keyring = kr
		if n, err := st.MoveToKeyring(kr); err != nil {
			fmt.Fprintln(stderr, "whiterosed:", err)
		} else if n > 0 {
			fmt.Fprintf(stderr, "whiterosed: moved %d secret(s) into the keyring\n", n)
		}
	} else {
		fmt.Fprintln(stderr, "whiterosed: secrets kept in the settings database:", err)
	}

	rep := migrate.Run(st, secrets, migrate.DefaultDirs())
	for _, f := range rep.Imported {
		fmt.Fprintf(stderr, "whiterosed: imported %s into the settings (kept as %s.migrated)\n", f, filepath.Base(f))
	}
	for f, e := range rep.Failed {
		fmt.Fprintf(stderr, "whiterosed: couldn't import %s (left as is): %v\n", f, e)
	}

	dirs := calendar.DefaultDirs()
	srv := &server.Server{Store: st, Secrets: secrets, Dirs: dirs}
	ln, err := net.Listen("unix", sock)
	if err != nil {
		fmt.Fprintln(stderr, "whiterosed:", err)
		return 1
	}
	_ = os.Chmod(sock, 0o600)
	// Health check (whiterose-launch): the daemon is up.
	if dir := os.Getenv("WHITEROSE_READY_DIR"); dir != "" {
		_ = os.WriteFile(filepath.Join(dir, "daemon"), nil, 0o600)
	}

	// System state: recomputed and pushed whenever something pokes.
	changed := make(chan struct{}, 1)
	poke := func() {
		select {
		case changed <- struct{}{}:
		default:
		}
	}
	read := func() SystemState {
		s := readState(os.DirFS(backlight.Sysfs), os.DirFS(rfkill.Sysfs))
		s.Weather, s.Calendars = weatherView(st, dirs.Cache), calendar.Version(dirs, localSources(st, dirs))
		return s
	}
	go func() {
		srv.SetState(read())
		for range changed {
			srv.SetState(read())
		}
	}()
	if f, err := os.Open(rfkill.Device); err == nil {
		go func() { _ = rfkill.ReadEvents(f, func(rfkill.Event) { poke() }) }()
	}
	go func() {
		_ = uevent.Listen(func(e *uevent.Event) {
			if e.Props["SUBSYSTEM"] == "backlight" || e.Props["SUBSYSTEM"] == "rfkill" {
				poke()
			}
		})
	}()
	go func() {
		err := session.Watch(func(r session.Request) { srv.Broadcast(map[string]string{"event": string(r)}) })
		fmt.Fprintln(stderr, "whiterosed: lock requests not watched:", err)
	}()
	// Lock before sleeping (privacy.lockOnSuspend): the shell locks, then
	// the inhibitor is let go a moment later.
	go func() {
		err := session.WatchSleep(time.Second, func() {
			var on bool
			if st.GetInto("privacy.lockOnSuspend", &on) == nil && on {
				srv.Broadcast(map[string]string{"event": string(session.Lock)})
			}
		})
		fmt.Fprintln(stderr, "whiterosed: no lock before sleep:", err)
	}()

	// Calendars every 15 minutes, weather every 30, and straight away when
	// their settings change.
	var calMu, wMu sync.Mutex
	refreshCalendars := func() {
		calMu.Lock()
		defer calMu.Unlock()
		_, errs := calendar.Refresh(context.Background(), dirs, srv.CalendarSources(), httpClient)
		for _, e := range errs {
			fmt.Fprintln(stderr, "whiterosed:", e)
		}
		poke()
	}
	refreshWeather := func() {
		wMu.Lock()
		defer wMu.Unlock()
		if _, err := weather.Refresh(context.Background(), weatherConfig(st), dirs.Cache, httpClient); err != nil && !errors.Is(err, weather.ErrNotConfigured) {
			fmt.Fprintln(stderr, "whiterosed: weather:", err)
		}
		poke()
	}
	srv.CalendarsChanged = refreshCalendars
	setup := &appSetup{
		broadcast: srv.Broadcast,
		done:      func() { _ = st.Set("apps.setupDone", json.RawMessage(`true`)) },
	}
	prints := &fingerprints{broadcast: srv.Broadcast}
	srv.Ops = map[string]func(server.Request) (any, error){
		"battery": func(server.Request) (any, error) {
			conn, err := dbus.ConnectSystemBus()
			if err != nil {
				return nil, err
			}
			defer conn.Close()
			return power.Read(conn, uint32(time.Now().Unix()))
		},
		"network_profile": func(r server.Request) (any, error) {
			conn, err := dbus.ConnectSystemBus()
			if err != nil {
				return nil, err
			}
			defer conn.Close()
			p, d, err := netconf.Get(conn, r.UUID)
			if err != nil {
				return nil, err
			}
			return map[string]any{"profile": p, "details": d}, nil
		},
		"network_update": func(r server.Request) (any, error) {
			var ch netconf.Changes
			if err := json.Unmarshal(r.Changes, &ch); err != nil {
				return nil, fmt.Errorf("bad changes: %w", err)
			}
			conn, err := dbus.ConnectSystemBus()
			if err != nil {
				return nil, err
			}
			defer conn.Close()
			return nil, netconf.Update(conn, r.UUID, ch)
		},
		// App setup (Settings > Apps, and the first-run prompt).
		"apps": func(server.Request) (any, error) { return setup.status() },
		"apps_install": func(r server.Request) (any, error) {
			var x struct {
				IDs []string `json:"ids"`
			}
			if err := json.Unmarshal(r.Args, &x); err != nil {
				return nil, err
			}
			return setup.install(x.IDs)
		},
		// Accounts (Users page). Each opens its own connection: a change can
		// wait on a polkit password prompt.
		"users": withAccounts(func(a *accounts.Service, r server.Request) (any, error) { return a.List() }),
		"user_name": withAccounts(func(a *accounts.Service, r server.Request) (any, error) {
			var x struct {
				UID  uint64 `json:"uid"`
				Name string `json:"name"`
			}
			if err := json.Unmarshal(r.Args, &x); err != nil {
				return nil, err
			}
			return nil, a.SetRealName(x.UID, x.Name)
		}),
		"user_icon": withAccounts(func(a *accounts.Service, r server.Request) (any, error) {
			var x struct {
				UID  uint64 `json:"uid"`
				Path string `json:"path"`
			}
			if err := json.Unmarshal(r.Args, &x); err != nil {
				return nil, err
			}
			return nil, a.SetIcon(x.UID, x.Path)
		}),
		"user_admin": withAccounts(func(a *accounts.Service, r server.Request) (any, error) {
			var x struct {
				UID   uint64 `json:"uid"`
				Admin bool   `json:"admin"`
			}
			if err := json.Unmarshal(r.Args, &x); err != nil {
				return nil, err
			}
			return nil, a.SetAdmin(x.UID, x.Admin)
		}),
		"user_password": withAccounts(func(a *accounts.Service, r server.Request) (any, error) {
			var x struct {
				UID      uint64 `json:"uid"`
				Password string `json:"password"`
			}
			if err := json.Unmarshal(r.Args, &x); err != nil {
				return nil, err
			}
			return nil, a.SetPassword(x.UID, x.Password)
		}),
		"user_create": withAccounts(func(a *accounts.Service, r server.Request) (any, error) {
			var x struct {
				Name     string `json:"name"`
				RealName string `json:"real_name"`
				Admin    bool   `json:"admin"`
				Password string `json:"password"`
			}
			if err := json.Unmarshal(r.Args, &x); err != nil {
				return nil, err
			}
			return a.Create(x.Name, x.RealName, x.Admin, x.Password)
		}),
		"user_delete": withAccounts(func(a *accounts.Service, r server.Request) (any, error) {
			var x struct {
				UID       uint64 `json:"uid"`
				KeepFiles bool   `json:"keep_files"`
			}
			if err := json.Unmarshal(r.Args, &x); err != nil {
				return nil, err
			}
			return nil, a.Delete(x.UID, x.KeepFiles)
		}),
		"set_charge_limit": func(r server.Request) (any, error) {
			conn, err := dbus.ConnectSystemBus()
			if err != nil {
				return nil, err
			}
			defer conn.Close()
			return nil, power.SetLimit(conn, r.On)
		},
	}
	// Sound (Settings › Sound): every place sound can go or come from,
	// including ones in another setup of the sound card.
	srv.Ops["audio_ports"] = func(server.Request) (any, error) { return audio.List(audio.Exec) }
	srv.Ops["audio_use"] = func(r server.Request) (any, error) {
		return nil, audio.Use(audio.Exec, r.Key, 3*time.Second)
	}
	// Fingerprints (Settings › Users › Fingerprints).
	for name, op := range prints.ops() {
		srv.Ops[name] = op
	}
	// WhiteRose updates (Settings › Updates, and the "couldn't start" prompt).
	updates := newUpdater(st, dirs.Cache, srv.Broadcast)
	for name, op := range updateOps(updates) {
		srv.Ops[name] = op
	}
	go scheduleUpdates(updates, st)
	every := func(d time.Duration, fn func()) {
		for {
			fn()
			time.Sleep(d)
		}
	}
	go every(15*time.Minute, refreshCalendars)
	go every(30*time.Minute, refreshWeather)
	changes, stopChanges := st.Subscribe()
	defer stopChanges()
	go func() {
		var t *time.Timer
		for c := range changes {
			switch {
			case strings.HasPrefix(c.Key, "weather."):
				// A city typed letter by letter: wait for a pause.
				if t != nil {
					t.Stop()
				}
				t = time.AfterFunc(time.Second, refreshWeather)
			case c.Key == "calendar.sources":
				poke()
			}
		}
	}()

	sigs := make(chan os.Signal, 1)
	signal.Notify(sigs, syscall.SIGTERM, syscall.SIGINT)
	go func() {
		<-sigs
		ln.Close()
	}()
	fmt.Fprintln(stdout, "whiterosed: serving", sock)
	err = srv.Serve(ln)
	_ = os.Remove(sock)
	if err != nil {
		fmt.Fprintln(stderr, "whiterosed:", err)
		return 1
	}
	return 0
}

// request sends one request to the running daemon and returns its reply.
func request(req map[string]any) (json.RawMessage, error) {
	c, err := net.Dial("unix", daemonSocket())
	if err != nil {
		return nil, errors.New("whiterosed isn't running (start it with `whiterosed serve`)")
	}
	defer c.Close()
	_ = c.SetDeadline(time.Now().Add(10 * time.Second))
	req["id"] = 1
	b, _ := json.Marshal(req)
	if _, err := c.Write(append(b, '\n')); err != nil {
		return nil, err
	}
	sc := bufio.NewScanner(c)
	sc.Buffer(make([]byte, 64*1024), 16<<20)
	for sc.Scan() {
		var m struct {
			Reply *int            `json:"reply"`
			Value json.RawMessage `json:"value"`
			Error string          `json:"error"`
		}
		if json.Unmarshal(sc.Bytes(), &m) != nil || m.Reply == nil || *m.Reply != 1 {
			continue
		}
		if m.Error != "" {
			return nil, errors.New(m.Error)
		}
		return m.Value, nil
	}
	return nil, errors.New("no reply from whiterosed")
}

func printJSON(w io.Writer, v json.RawMessage) {
	var x any
	if json.Unmarshal(v, &x) != nil {
		fmt.Fprintln(w, string(v))
		return
	}
	b, _ := json.MarshalIndent(x, "", "  ")
	fmt.Fprintln(w, string(b))
}

// cmdSettings reads and changes settings through the running daemon, so
// every open panel sees the change at once.
func cmdSettings(args []string, stdout, stderr io.Writer) int {
	usage := "usage: whiterosed settings list | get KEY | set KEY JSON | reset KEY"
	var req map[string]any
	switch {
	case len(args) == 1 && args[0] == "list":
		req = map[string]any{"op": "all"}
	case len(args) == 2 && args[0] == "get":
		req = map[string]any{"op": "get", "key": args[1]}
	case len(args) == 3 && args[0] == "set":
		if !json.Valid([]byte(args[2])) {
			fmt.Fprintf(stderr, "whiterosed: the value must be JSON (a word needs quotes: '\"London\"')\n")
			return 2
		}
		req = map[string]any{"op": "set", "key": args[1], "value": json.RawMessage(args[2])}
	case len(args) == 2 && args[0] == "reset":
		req = map[string]any{"op": "reset", "key": args[1]}
	default:
		fmt.Fprintln(stderr, usage)
		return 2
	}
	v, err := request(req)
	if err != nil {
		fmt.Fprintln(stderr, "whiterosed:", err)
		return 1
	}
	if args[0] == "list" {
		var all map[string]json.RawMessage
		_ = json.Unmarshal(v, &all)
		for _, k := range store.Keys(all) {
			fmt.Fprintf(stdout, "%s = %s\n", k, all[k])
		}
		return 0
	}
	if v != nil {
		printJSON(stdout, v)
	}
	return 0
}

// cmdCalendar manages calendars through the running daemon.
func cmdCalendar(args []string, stdout, stderr io.Writer) int {
	usage := "usage: whiterosed calendar list | add NAME (URL|FILE) [#COLOUR] | remove NUMBER | events FROM TO"
	var req map[string]any
	switch {
	case len(args) == 1 && args[0] == "list":
		req = map[string]any{"op": "calendars"}
	case (len(args) == 3 || len(args) == 4) && args[0] == "add":
		req = map[string]any{"op": "add_calendar", "name": args[1]}
		if strings.Contains(args[2], "://") {
			req["url"] = args[2]
		} else {
			p, _ := filepath.Abs(args[2])
			req["path"] = p
		}
		if len(args) == 4 {
			req["color"] = args[3]
		}
	case len(args) == 2 && args[0] == "remove":
		var n int
		if _, err := fmt.Sscan(args[1], &n); err != nil || n < 1 {
			fmt.Fprintln(stderr, usage)
			return 2
		}
		req = map[string]any{"op": "remove_calendar", "index": n - 1}
	case len(args) == 3 && args[0] == "events":
		req = map[string]any{"op": "calendar_events", "from": args[1], "to": args[2]}
	default:
		fmt.Fprintln(stderr, usage)
		return 2
	}
	v, err := request(req)
	if err != nil {
		fmt.Fprintln(stderr, "whiterosed:", err)
		return 1
	}
	if args[0] == "list" {
		var cals []struct {
			Name    string `json:"name"`
			Color   string `json:"color"`
			Path    string `json:"path"`
			HasLink bool   `json:"has_link"`
		}
		_ = json.Unmarshal(v, &cals)
		for i, c := range cals {
			src := c.Path
			if c.HasLink {
				src = "(link, in the keyring)"
			}
			fmt.Fprintf(stdout, "%d. %s  %s  %s\n", i+1, c.Name, c.Color, src)
		}
		return 0
	}
	if v != nil {
		printJSON(stdout, v)
	}
	return 0
}

// withAccounts gives an op its own AccountsService connection.
func withAccounts(fn func(*accounts.Service, server.Request) (any, error)) func(server.Request) (any, error) {
	return func(r server.Request) (any, error) {
		a, err := accounts.New(uint64(os.Getuid()))
		if err != nil {
			return nil, err
		}
		defer a.Close()
		return fn(a, r)
	}
}
