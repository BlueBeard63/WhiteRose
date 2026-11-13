package main

import (
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
	"time"

	"whiterose/daemon/internal/crash"
	"whiterose/daemon/internal/memwatch"
	"whiterose/daemon/internal/server"
)

// A fake app.slice and /proc: `apps` unit → [MB, pid...].
func fakeMemory(t *testing.T, availMB int, psi string, apps map[string][]int) *memWatch {
	t.Helper()
	root := t.TempDir()
	slice, proc := filepath.Join(root, "app.slice"), filepath.Join(root, "proc")
	os.MkdirAll(filepath.Join(proc, "pressure"), 0o755)
	os.WriteFile(filepath.Join(proc, "meminfo"), []byte("MemTotal: "+strconv.Itoa(16*1024*1024)+" kB\nMemAvailable: "+strconv.Itoa(availMB*1024)+" kB\n"), 0o644)
	os.WriteFile(filepath.Join(proc, "pressure", "memory"), []byte(psi), 0o644)
	var events []any
	m := &memWatch{slice: slice, proc: proc, terminal: "ptyxis", now: time.Now,
		raised: map[string]bool{}, snoozed: map[string]time.Time{}, warned: map[string]time.Time{}, wake: make(chan struct{}, 1),
		broadcast: func(e any) { events = append(events, e) }}
	for unit, v := range apps {
		d := filepath.Join(slice, unit)
		os.MkdirAll(d, 0o755)
		os.WriteFile(filepath.Join(d, "memory.current"), []byte(strconv.Itoa(v[0]<<20)), 0o644)
		os.WriteFile(filepath.Join(d, "cgroup.kill"), nil, 0o644) // (the kernel's file)
		var pids []string
		for _, p := range v[1:] {
			pids = append(pids, strconv.Itoa(p))
			os.MkdirAll(filepath.Join(proc, strconv.Itoa(p)), 0o755)
			os.WriteFile(filepath.Join(proc, strconv.Itoa(p), "oom_score_adj"), []byte("300"), 0o644)
		}
		os.WriteFile(filepath.Join(d, "cgroup.procs"), []byte(strings.Join(pids, "\n")+"\n"), 0o644)
	}
	t.Cleanup(func() { _ = events })
	return m
}

func reqName(n string) server.Request { return server.Request{Name: n} }

func TestMemoryWarns(t *testing.T) {
	calm := "some avg10=0.00 avg60=0 avg300=0 total=0\nfull avg10=0.00 avg60=0 avg300=0 total=0\n"
	apps := map[string][]int{
		"app-gnome-firefox-1.scope":                    {9000, 101, 102}, // more than half of 16 GB
		"app-whiterose-org.gnome.Ptyxis-1-0.scope":     {12000, 103},     // bigger, but the terminal
		"app-whiterose-org.gnome.Calculator-1-1.scope": {80, 104},
		`app-gnome\x2dsession\x2dmanager.slice`:        {50},
	}
	var got []any
	m := fakeMemory(t, 8000, calm, apps)
	m.broadcast = func(e any) { got = append(got, e) }
	m.check()
	if m.level.String() != "calm" || len(got) != 0 {
		t.Fatalf("calm: %v %v", m.level, got)
	}
	// Firefox holds over half: a suspect, first in the kill order (all its processes).
	for _, pid := range []string{"101", "102"} {
		if b, _ := os.ReadFile(filepath.Join(m.proc, pid, "oom_score_adj")); string(b) != "800" {
			t.Errorf("pid %s: %s", pid, b)
		}
	}
	if b, _ := os.ReadFile(filepath.Join(m.proc, "103", "oom_score_adj")); string(b) != "300" {
		t.Errorf("the terminal was raised: %s", b)
	}

	// Tight (1 GB of 16 left): Firefox is named, once, and not after Not now.
	os.WriteFile(filepath.Join(m.proc, "meminfo"), []byte("MemTotal: 16777216 kB\nMemAvailable: 1048576 kB\n"), 0o644)
	m.check()
	m.check()
	if len(got) != 1 {
		t.Fatalf("warnings: %v", got)
	}
	w := got[0].(map[string]any)
	if app, ok := w["app"].(memwatch.Suspect); w["level"] != "tight" || !ok || app.ID != "firefox" || app.Reason != "half" {
		t.Fatalf("%+v", w)
	}
	ops := m.ops()
	// A shell connecting now asks for it.
	if cur, _ := ops["memory_warning"](reqName("")); cur == nil {
		t.Error("no current warning")
	}
	ops["memory_snooze"](reqName("app-gnome-firefox-1.scope"))
	if cur, _ := ops["memory_warning"](reqName("")); cur != nil {
		t.Errorf("answered, yet %v", cur)
	}
	m.warned = map[string]time.Time{}
	m.check()
	if len(got) != 1 {
		t.Errorf("named after Not now: %v", got)
	}
	st, _ := ops["memory_status"](reqName(""))
	s := st.(map[string]any)
	if s["level"] != "tight" || s["total"].(int64) != 16<<30 {
		t.Errorf("%v", s)
	}
	b, _ := json.Marshal(s["apps"])
	if !strings.Contains(string(b), `"id":"org.gnome.Ptyxis","bytes":12582912000,"growing":false,"suspect":false,"kept":"terminal"`) ||
		!strings.Contains(string(b), `"id":"firefox","bytes":9437184000,"growing":false,"suspect":true,"kept":""`) {
		t.Errorf("%s", b)
	}
}

func TestMemoryClose(t *testing.T) {
	// A real process to close.
	cmd := exec.Command("sleep", "30")
	if err := cmd.Start(); err != nil {
		t.Fatal(err)
	}
	done := make(chan struct{})
	go func() { cmd.Wait(); close(done) }()
	m := fakeMemory(t, 8000, "", map[string][]int{"app-gnome-sleep-1.scope": {10, cmd.Process.Pid}})
	if err := m.close("../../etc", time.Second); err == nil {
		t.Error("closed a path")
	}
	if err := m.close("app-gnome-gone-2.scope", time.Second); err == nil {
		t.Error("closed an app that isn't there")
	}
	if err := m.close("app-gnome-sleep-1.scope", 50*time.Millisecond); err != nil {
		t.Fatal(err)
	}
	select {
	case <-done:
	case <-time.After(2 * time.Second):
		cmd.Process.Kill()
		t.Fatal("not closed")
	}
	// Still listed after the wait: the scope's cgroup.kill is written.
	time.Sleep(150 * time.Millisecond)
	if b, _ := os.ReadFile(filepath.Join(m.slice, "app-gnome-sleep-1.scope", "cgroup.kill")); string(b) != "1" {
		t.Errorf("cgroup.kill %q", b)
	}
}

// Critical: the worst unprotected app is closed by itself, once, with a
// crash record; protected apps never; not with memory.autoClose off.
func TestMemoryCriticalCloses(t *testing.T) {
	stall := "some avg10=60.00 avg60=0 avg300=0 total=0\nfull avg10=35.00 avg60=0 avg300=0 total=0\n"
	apps := map[string][]int{
		"app-gnome-firefox-1.scope":                {3000},
		"app-whiterose-org.gnome.Ptyxis-1-0.scope": {6000}, // the terminal: bigger, never closed
	}
	m := fakeMemory(t, 4000, stall, apps)
	m.state = t.TempDir()
	m.version = "0.3.1"
	var got []map[string]any
	m.broadcast = func(e any) { got = append(got, e.(map[string]any)) }
	m.check()
	if len(got) != 1 || got[0]["event"] != "memory_closed" {
		t.Fatalf("%v", got)
	}
	if app := got[0]["app"].(memwatch.Suspect); app.ID != "firefox" {
		t.Errorf("closed %+v", app)
	}
	recs := crash.List(m.state)
	if len(recs) != 1 || recs[0].What != crash.Memory || recs[0].App != "firefox" || got[0]["record"] != recs[0].ID {
		t.Errorf("records %+v, event %v", recs, got[0]["record"])
	}
	if recs[0].Describe() != "Closed firefox to free memory" {
		t.Errorf("%q", recs[0].Describe())
	}
	// Right after: not another one (the memory needs time to come back).
	m.check()
	if len(got) != 1 {
		t.Errorf("closed again straight away: %v", got)
	}
	// A shell connecting now hears about it.
	if cur, _ := m.ops()["memory_warning"](reqName("")); cur.(map[string]any)["event"] != "memory_closed" {
		t.Errorf("%v", cur)
	}

	// Only the terminal left: nothing is closed.
	m2 := fakeMemory(t, 4000, stall, map[string][]int{"app-whiterose-org.gnome.Ptyxis-1-0.scope": {6000}})
	m2.state = t.TempDir()
	var got2 []any
	m2.broadcast = func(e any) { got2 = append(got2, e) }
	m2.check()
	if len(got2) != 0 || len(crash.List(m2.state)) != 0 {
		t.Errorf("closed a protected app: %v", got2)
	}
}
