package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"os"
	"strconv"
	"sync"

	"whiterose/daemon/internal/backlight"
	"whiterose/daemon/internal/calendar"
	"whiterose/daemon/internal/rfkill"
	"whiterose/daemon/internal/session"
	"whiterose/daemon/internal/uevent"
	"whiterose/daemon/internal/weather"
)

// SystemState is what the quick settings panel shows from the daemon.
type SystemState struct {
	// Brightness is 0-100, or nil without a backlight (desktop monitors).
	Brightness *int   `json:"brightness"`
	Backlight  string `json:"backlight,omitempty"`
	Airplane   bool   `json:"airplane"`
	Radios     int    `json:"radios"`
	// Current conditions for the configured place, or nil.
	Weather *weather.Now `json:"weather"`
	// Changes whenever the calendars might have: the shell re-reads events.
	Calendars int64 `json:"calendars"`
}

func readState(backlights, radios fs.FS) SystemState {
	var s SystemState
	if d, err := backlight.Find(backlights); err == nil {
		p := d.Percent()
		s.Brightness, s.Backlight = &p, d.Name
	}
	if r, err := rfkill.Scan(radios); err == nil {
		s.Airplane, s.Radios = rfkill.Airplane(r), len(r)
	}
	return s
}

// emitChanges writes the state as one JSON line now and again after each
// signal on changed, skipping repeats (one rfkill toggle fires several events).
func emitChanges(changed <-chan struct{}, read func() SystemState, out io.Writer) error {
	enc := json.NewEncoder(out)
	var last []byte
	emit := func() error {
		b, err := json.Marshal(read())
		if err != nil || string(b) == string(last) {
			return err
		}
		last = b
		return enc.Encode(json.RawMessage(b))
	}
	if err := emit(); err != nil {
		return err
	}
	for range changed {
		if err := emit(); err != nil {
			return err
		}
	}
	return nil
}

// syncWriter lets the state stream and lock requests share stdout without
// interleaving lines.
type syncWriter struct {
	mu sync.Mutex
	w  io.Writer
}

func (s *syncWriter) Write(p []byte) (int, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.w.Write(p)
}

// lockLine is what watch prints when logind asks the session to lock/unlock.
func lockLine(r session.Request) string { return `{"event":"` + string(r) + `"}` + "\n" }

// cmdWatch prints the system state as JSON lines whenever it changes, and
// {"event":"lock"|"unlock"} when logind asks. It blocks in the kernel and on
// D-Bus between changes (rfkill reads, netlink uevents, logind signals).
func cmdWatch(_ []string, stdout, stderr io.Writer) int {
	out := &syncWriter{w: stdout}
	go func() {
		err := session.Watch(func(r session.Request) { _, _ = io.WriteString(out, lockLine(r)) })
		fmt.Fprintln(stderr, "whiterosed: lock requests not watched:", err)
	}()
	changed := make(chan struct{}, 1)
	poke := func() {
		select {
		case changed <- struct{}{}:
		default:
		}
	}
	if f, err := os.Open(rfkill.Device); err == nil {
		go func() { _ = rfkill.ReadEvents(f, func(rfkill.Event) { poke() }) }()
	} else {
		fmt.Fprintln(stderr, "whiterosed: radios not watched:", err)
	}
	go func() {
		err := uevent.Listen(func(e *uevent.Event) {
			if e.Props["SUBSYSTEM"] == "backlight" || e.Props["SUBSYSTEM"] == "rfkill" {
				poke()
			}
		})
		fmt.Fprintln(stderr, "whiterosed: brightness not watched:", err)
	}()
	startRefreshers(stderr, poke)
	dirs := calendar.DefaultDirs()
	read := func() SystemState {
		s := readState(os.DirFS(backlight.Sysfs), os.DirFS(rfkill.Sysfs))
		s.Weather, s.Calendars = weatherView(dirs), calendar.Version(dirs)
		return s
	}
	if err := emitChanges(changed, read, out); err != nil {
		fmt.Fprintln(stderr, "whiterosed:", err)
		return 1
	}
	return 0
}

func cmdBrightness(args []string, stdout, stderr io.Writer) int {
	if len(args) != 2 || args[0] != "set" {
		fmt.Fprintln(stderr, "usage: whiterosed brightness set <percent>")
		return 2
	}
	pct, err := strconv.Atoi(args[1])
	if err != nil || pct < 1 || pct > 100 {
		fmt.Fprintln(stderr, "whiterosed: brightness must be 1-100")
		return 2
	}
	d, err := backlight.Find(os.DirFS(backlight.Sysfs))
	if err == nil {
		err = backlight.Set(d, pct)
	}
	if err != nil {
		fmt.Fprintln(stderr, "whiterosed: brightness:", err)
		if errors.Is(err, backlight.ErrNone) {
			return 3
		}
		return 1
	}
	return 0
}

func cmdAirplane(args []string, stdout, stderr io.Writer) int {
	if len(args) != 1 || (args[0] != "on" && args[0] != "off") {
		fmt.Fprintln(stderr, "usage: whiterosed airplane on|off")
		return 2
	}
	if err := rfkill.SetAll(args[0] == "on"); err != nil {
		fmt.Fprintln(stderr, "whiterosed: airplane mode:", err)
		return 1
	}
	return 0
}
