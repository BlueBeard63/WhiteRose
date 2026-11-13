package main

// The memory watcher (docs/plans/memory-protection.md; the judging is in
// internal/memwatch). It reads each app scope's memory every 30 s (a few
// small files) to see what grows, and every 3 s while memory is tight. A
// PSI trigger on /proc/pressure/memory wakes it as soon as things start
// waiting on memory, so it doesn't have to look often while it's calm.
//
//   - An app growing steadily for 10 minutes while not focused, or holding
//     more than half of memory, is a suspect: its processes' oom_score_adj
//     goes to 800, first in line if the kernel has to kill.
//   - Tight: the island names the worst app ("Firefox is using 6.2 GB and
//     still growing") with Close and Not now (memory_warning events).
//   - Protected apps (memory.protected, and the terminal) are never named,
//     raised or closed.

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"sync"
	"syscall"
	"time"

	"golang.org/x/sys/unix"

	"whiterose/daemon/internal/memwatch"
	"whiterose/daemon/internal/server"
	"whiterose/daemon/internal/store"
)

type memWatch struct {
	slice     string // the user's app.slice in the cgroup tree
	proc      string // /proc
	st        *store.Store
	broadcast func(any)
	terminal  string
	now       func() time.Time

	mu       sync.Mutex
	tr       memwatch.Tracker
	level    memwatch.Level
	apps     []memwatch.App
	raised   map[string]bool      // units already at the suspect score
	snoozed  map[string]time.Time // Not now: not named again until
	warned   map[string]time.Time // last named
	lastRead time.Time
	wake     chan struct{}
	// The warning showing (nil: none): a shell that connects, or restarts,
	// asks for it (memory_warning).
	current map[string]any
}

func newMemWatch(st *store.Store, broadcast func(any)) *memWatch {
	uid := strconv.Itoa(os.Getuid())
	slice := filepath.Join("/sys/fs/cgroup/user.slice", "user-"+uid+".slice", "user@"+uid+".service", "app.slice")
	if appSliceOverride != "" {
		slice = appSliceOverride
	}
	return &memWatch{
		slice: slice,
		proc:  "/proc", st: st, broadcast: broadcast, terminal: os.Getenv("TERMINAL"), now: time.Now,
		raised: map[string]bool{}, snoozed: map[string]time.Time{}, warned: map[string]time.Time{},
		wake: make(chan struct{}, 1),
	}
}

// (Trying it out: WHITEROSE_APP_SLICE points the watcher at a made-up
// app.slice, so nothing real is named or closed.)
func init() {
	if d := os.Getenv("WHITEROSE_APP_SLICE"); d != "" {
		appSliceOverride = d
	}
}

var appSliceOverride string

func (m *memWatch) protected(id string) bool {
	var list []string
	if m.st != nil {
		_ = m.st.GetInto("memory.protected", &list)
	}
	return memwatch.Protected(id, list, m.terminal)
}

// readApps: every app scope and its memory now.
func (m *memWatch) readApps() []memwatch.App {
	entries, _ := os.ReadDir(m.slice)
	var out []memwatch.App
	for _, e := range entries {
		id, ok := memwatch.ParseUnit(e.Name())
		if !ok || !e.IsDir() {
			continue
		}
		b, err := os.ReadFile(filepath.Join(m.slice, e.Name(), "memory.current"))
		if err != nil {
			continue
		}
		n, _ := strconv.ParseInt(strings.TrimSpace(string(b)), 10, 64)
		out = append(out, memwatch.App{Unit: e.Name(), ID: id, Bytes: n})
	}
	sort.Slice(out, func(i, j int) bool { return out[i].Bytes > out[j].Bytes })
	return out
}

// readMemory: available and total (bytes), and the pressure.
func (m *memWatch) readMemory() (avail, total int64, p memwatch.PSI) {
	if b, err := os.ReadFile(filepath.Join(m.proc, "meminfo")); err == nil {
		for l := range strings.SplitSeq(string(b), "\n") {
			f := strings.Fields(l)
			if len(f) < 2 {
				continue
			}
			n, _ := strconv.ParseInt(f[1], 10, 64)
			switch f[0] {
			case "MemAvailable:":
				avail = n << 10
			case "MemTotal:":
				total = n << 10
			}
		}
	}
	if b, err := os.ReadFile(filepath.Join(m.proc, "pressure/memory")); err == nil {
		p = memwatch.ParsePSI(string(b))
	}
	return avail, total, p
}

// focusedApp: the app id with the keyboard (from the compositor), "" if
// unknown. Asked only when something looks like it's growing.
func focusedApp() string {
	w, _ := compositorState()
	if w == nil {
		return ""
	}
	var ev struct {
		Windows []struct {
			AppID   *string `json:"app_id"`
			Focused bool    `json:"focused"`
		} `json:"windows"`
	}
	if json.Unmarshal(w, &ev) != nil {
		return ""
	}
	for _, x := range ev.Windows {
		if x.Focused && x.AppID != nil {
			return *x.AppID
		}
	}
	return ""
}

// check reads everything once and acts on it.
func (m *memWatch) check() {
	apps := m.readApps()
	avail, total, psi := m.readMemory()
	now := m.now()

	m.mu.Lock()
	m.tr.Add(now, apps)
	m.apps, m.lastRead = apps, now
	m.level = memwatch.Judge(psi, avail, total)
	// (Trying it out: WHITEROSE_MEMORY_LEVEL=tight acts as if memory were
	// tight, with the real apps and their real sizes.)
	if os.Getenv("WHITEROSE_MEMORY_LEVEL") == "tight" && m.level < memwatch.Tight {
		m.level = memwatch.Tight
	}
	growing := false
	for _, a := range apps {
		growing = growing || m.tr.Growing(a.Unit)
	}
	level := m.level
	m.mu.Unlock()

	focused := ""
	if growing || level >= memwatch.Tight {
		focused = focusedApp()
	}

	m.mu.Lock()
	defer m.mu.Unlock()
	// Suspects go to the front of the kill order.
	for _, s := range m.tr.Suspects(apps, total, focused, m.protected) {
		if !m.raised[s.Unit] {
			m.raised[s.Unit] = true
			m.setScore(s.Unit, 800)
		}
	}
	for u := range m.raised {
		if !hasApp(apps, u) {
			delete(m.raised, u)
		}
	}
	if level < memwatch.Tight {
		m.current = nil
		return
	}
	if m.current != nil && !hasApp(apps, m.current["app"].(memwatch.Suspect).Unit) {
		m.current = nil // closed by itself
	}
	w, ok := m.tr.Worst(apps, total, focused, m.protected)
	if !ok || now.Before(m.snoozed[w.Unit]) || now.Sub(m.warned[w.Unit]) < 2*time.Minute {
		return
	}
	m.warned[w.Unit] = now
	m.current = map[string]any{"event": "memory_warning", "level": level.String(), "app": w,
		"available": avail, "total": total}
	if m.broadcast != nil {
		m.broadcast(m.current)
	}
}

func hasApp(apps []memwatch.App, unit string) bool {
	for _, a := range apps {
		if a.Unit == unit {
			return true
		}
	}
	return false
}

// pids: the processes in an app scope.
func (m *memWatch) pids(unit string) []int {
	b, err := os.ReadFile(filepath.Join(m.slice, unit, "cgroup.procs"))
	if err != nil {
		return nil
	}
	var out []int
	for l := range strings.SplitSeq(strings.TrimSpace(string(b)), "\n") {
		if n, err := strconv.Atoi(l); err == nil {
			out = append(out, n)
		}
	}
	return out
}

func (m *memWatch) setScore(unit string, adj int) {
	for _, pid := range m.pids(unit) {
		// Raising needs no privileges; a process that's gone is fine.
		_ = os.WriteFile(filepath.Join(m.proc, strconv.Itoa(pid), "oom_score_adj"), []byte(strconv.Itoa(adj)), 0)
	}
}

// validUnit: an app scope that exists in the slice (no paths).
func (m *memWatch) validUnit(unit string) error {
	if _, ok := memwatch.ParseUnit(unit); !ok || strings.ContainsAny(unit, "/") {
		return errors.New("not an app")
	}
	if _, err := os.Stat(filepath.Join(m.slice, unit)); err != nil {
		return errors.New("that app has already closed")
	}
	return nil
}

// close asks an app to close (SIGTERM to each of its processes), and makes
// it (the scope's cgroup.kill) if it's still there after `wait`.
func (m *memWatch) close(unit string, wait time.Duration) error {
	if err := m.validUnit(unit); err != nil {
		return err
	}
	for _, pid := range m.pids(unit) {
		_ = syscall.Kill(pid, syscall.SIGTERM)
	}
	go func() {
		time.Sleep(wait)
		if len(m.pids(unit)) > 0 {
			_ = os.WriteFile(filepath.Join(m.slice, unit, "cgroup.kill"), []byte("1"), 0)
		}
		m.poke()
	}()
	return nil
}

// answered: the island's warning was answered (Close or Not now).
func (m *memWatch) answered() {
	m.mu.Lock()
	m.current = nil
	m.mu.Unlock()
}

func (m *memWatch) poke() {
	select {
	case m.wake <- struct{}{}:
	default:
	}
}

// psiTrigger wakes the watcher when tasks stall on memory for 150 ms in a
// 2 s window (the shortest window an unprivileged trigger may use). Without
// triggers (an old kernel), the 30 s reading catches it, later.
func (m *memWatch) psiTrigger() {
	fd, err := unix.Open(filepath.Join(m.proc, "pressure/memory"), unix.O_RDWR|unix.O_NONBLOCK|unix.O_CLOEXEC, 0)
	if err != nil {
		return
	}
	if _, err := unix.Write(fd, []byte("some 150000 2000000\x00")); err != nil {
		unix.Close(fd)
		return
	}
	go func() {
		defer unix.Close(fd)
		fds := []unix.PollFd{{Fd: int32(fd), Events: unix.POLLPRI}}
		for {
			n, err := unix.Poll(fds, -1)
			if err != nil && err != unix.EINTR {
				return
			}
			if n > 0 {
				if fds[0].Revents&unix.POLLERR != 0 {
					return
				}
				m.poke()
			}
		}
	}()
}

func (m *memWatch) run() {
	m.psiTrigger()
	for {
		m.check()
		m.mu.Lock()
		every := 30 * time.Second
		if m.level >= memwatch.Tight {
			every = 3 * time.Second
		}
		m.mu.Unlock()
		select {
		case <-m.wake:
		case <-time.After(every):
		}
	}
}

func (m *memWatch) ops() map[string]func(server.Request) (any, error) {
	return map[string]func(server.Request) (any, error){
		// The island's Close: ask the app to close, make it after 3 s.
		"memory_close": func(r server.Request) (any, error) {
			m.answered()
			return nil, m.close(r.Name, 3*time.Second)
		},
		// Not now: not named again for 10 minutes.
		"memory_snooze": func(r server.Request) (any, error) {
			m.answered()
			m.mu.Lock()
			defer m.mu.Unlock()
			m.snoozed[r.Name] = m.now().Add(10 * time.Minute)
			return nil, nil
		},
		// The warning showing, if any (a shell that just connected).
		"memory_warning": func(server.Request) (any, error) {
			m.mu.Lock()
			defer m.mu.Unlock()
			if m.current == nil {
				return nil, nil
			}
			return m.current, nil
		},
		// The Settings page: how it stands, and each app's memory.
		"memory_status": func(server.Request) (any, error) {
			m.mu.Lock()
			defer m.mu.Unlock()
			type row struct {
				memwatch.App
				Growing   bool `json:"growing"`
				Protected bool `json:"protected"`
				Suspect   bool `json:"suspect"`
			}
			rows := []row{}
			for _, a := range m.apps {
				rows = append(rows, row{App: a, Growing: m.tr.Growing(a.Unit), Protected: m.protected(a.ID), Suspect: m.raised[a.Unit]})
			}
			return map[string]any{"level": m.level.String(), "apps": rows, "read": m.lastRead.UnixMilli()}, nil
		},
	}
}
