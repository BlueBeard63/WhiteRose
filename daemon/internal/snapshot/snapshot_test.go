package snapshot

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func write(t *testing.T, path, text string) {
	t.Helper()
	os.MkdirAll(filepath.Dir(path), 0o755)
	if err := os.WriteFile(path, []byte(text), 0o644); err != nil {
		t.Fatal(err)
	}
}

// A process: utime/stime in ticks, rss in pages.
func stat(pid int, comm string, ticks, rssPages int) string {
	return fmt.Sprintf("%d (%s) S 1 1 1 0 -1 0 0 0 0 0 %d 0 0 0 20 0 1 0 100 1000 %d 0", pid, comm, ticks, rssPages)
}

func TestTake(t *testing.T) {
	root := t.TempDir()
	proc, sys := filepath.Join(root, "proc"), filepath.Join(root, "sys")
	write(t, filepath.Join(proc, "sys/kernel/osrelease"), "6.17.0-5-generic\n")
	write(t, filepath.Join(proc, "cpuinfo"), "processor\t: 0\nmodel name\t: AMD Ryzen 7 7840U\n")
	write(t, filepath.Join(proc, "meminfo"), "MemTotal: 16000000 kB\nMemAvailable: 6000000 kB\nSwapTotal: 4000000 kB\nSwapFree: 3000000 kB\n")
	write(t, filepath.Join(proc, "loadavg"), "1.50 0.80 0.40 2/900 1234\n")
	write(t, filepath.Join(proc, "10/stat"), stat(10, "firefox", 100, 300000))
	write(t, filepath.Join(proc, "11/stat"), stat(11, "Xwayland", 5, 1000))
	write(t, filepath.Join(proc, "12/stat"), stat(12, "Web Content (x)", 0, 500))
	// A kernel thread (parent kthreadd): left out.
	write(t, filepath.Join(proc, "13/stat"), strings.Replace(stat(13, "kworker/6:0-i915", 999, 999999), " S 1 ", " I 2 ", 1))
	write(t, filepath.Join(sys, "class/drm/card1/device/vendor"), "0x1002\n")
	write(t, filepath.Join(sys, "class/drm/card1/device/device"), "0x15bf\n")
	os.MkdirAll(filepath.Join(sys, "class/drm/card1-eDP-1"), 0o755)
	osr := filepath.Join(root, "os-release")
	write(t, osr, "NAME=Ubuntu\nPRETTY_NAME=\"Ubuntu 26.04 LTS\"\n")
	logf := filepath.Join(root, "session.log")
	now := time.Date(2026, 11, 13, 13, 0, 0, 0, time.Local)
	var log strings.Builder
	log.WriteString("WhiteRose 0.3.0, session started 2026-11-13 12:00:00\n")
	for i := range 10 {
		fmt.Fprintf(&log, "line %d\n", i)
	}
	write(t, logf, log.String())

	src := Sources{
		Proc: proc, Sys: sys, OSRelease: osr, SessionLog: logf, Version: "0.3.0",
		Sample: 20 * time.Millisecond, LogLines: 3, Now: func() time.Time { return now },
		Compositor: func() (json.RawMessage, json.RawMessage) {
			return json.RawMessage(`{"event":"windows","windows":[
					{"id":1,"app_id":"firefox","title":"secret bank page","workspace":1,"focused":true},
					{"id":2,"app_id":"firefox","title":"x","workspace":2,"focused":false},
					{"id":3,"app_id":null,"title":"y","workspace":2,"focused":false}]}`),
				json.RawMessage(`{"event":"outputs","outputs":[
					{"make":"BOE","model":"0x0BCA","builtin":true,"enabled":true,"scale":1.5,"mode":{"width":2880,"height":1800,"refresh":120000,"preferred":true}},
					{"make":"Dell","model":"U2723QE","enabled":false,"scale":1}]}`)
		},
	}
	s := Take(src, false)
	if s.System.OS != "Ubuntu 26.04 LTS" || s.System.Kernel != "6.17.0-5-generic" || s.System.CPU != "AMD Ryzen 7 7840U" || s.System.MemoryMB != 15625 {
		t.Errorf("system %+v", s.System)
	}
	if len(s.System.GPUs) != 1 || s.System.GPUs[0].Vendor != "AMD" || s.System.GPUs[0].Device != "0x15bf" {
		t.Errorf("gpus %+v", s.System.GPUs)
	}
	if len(s.System.Monitors) != 1 || s.System.Monitors[0].Mode != "2880x1800 @ 120 Hz" || !s.System.Monitors[0].Builtin {
		t.Errorf("monitors %+v", s.System.Monitors)
	}
	if !s.WhiteRose.Xwayland || s.WhiteRose.Uptime != 3600 || s.WhiteRose.Version != "0.3.0" {
		t.Errorf("whiterose %+v", s.WhiteRose)
	}
	if s.Load == nil || s.Load.Avg[0] != 1.5 || s.Load.MemUsedMB != 9765 || s.Load.SwapUsedMB != 976 {
		t.Fatalf("load %+v", s.Load)
	}
	for _, p := range append(s.Load.TopCPU, s.Load.TopMemory...) {
		if strings.HasPrefix(p.Name, "kworker") {
			t.Errorf("a kernel thread: %+v", p)
		}
	}
	if len(s.Load.TopMemory) == 0 || s.Load.TopMemory[0].Name != "firefox" || s.Load.TopMemory[2].Name != "Web Content (x)" {
		t.Errorf("top memory %+v", s.Load.TopMemory)
	}
	w := s.Windows
	if w == nil || w.Count != 3 || w.Apps["firefox"] != 2 || w.Apps["(no app ID)"] != 1 || w.PerWorkspace[2] != 2 || w.Focused != "firefox" {
		t.Errorf("windows %+v", w)
	}
	if strings.Join(s.Log, "|") != "line 7|line 8|line 9" {
		t.Errorf("log %q", s.Log)
	}
	// Titles never get in.
	b, _ := json.Marshal(s)
	if strings.Contains(string(b), "secret bank page") {
		t.Error("a window title is in the snapshot")
	}

	// Late: no load or windows.
	if l := Take(src, true); l.Load != nil || l.Windows != nil || !l.Late || len(l.System.Monitors) != 1 {
		t.Errorf("late %+v", l)
	}
}

func TestCPUSample(t *testing.T) {
	proc := t.TempDir()
	write(t, filepath.Join(proc, "7/stat"), stat(7, "busy", 0, 10))
	go func() {
		time.Sleep(10 * time.Millisecond)
		write(t, filepath.Join(proc, "7/stat"), stat(7, "busy", 10, 10))
	}()
	l := load(Sources{Proc: proc, Sample: 100 * time.Millisecond})
	// 10 ticks (0.1 s of CPU) in 0.1 s: one whole core.
	if len(l.TopCPU) != 1 || l.TopCPU[0].CPU != 100 {
		t.Errorf("%+v", l.TopCPU)
	}
}
