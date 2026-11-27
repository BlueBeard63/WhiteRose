//go:build uitest

package uitest_test

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"whiterose/daemon/internal/uitest"
)

func tasksBinary(s *uitest.Session) string {
	if path := os.Getenv("WR_UITEST_TASKS"); path != "" {
		return path
	}
	return filepath.Join(s.Repo, "apps/target/debug/whiterose-tasks")
}

// tasksFixture lays out a made-up /proc, /sys and app.slice so the sampler
// and the memory watcher read one controllable app ("Demo") and nothing
// real. Returned as environment that points both subsystems at it.
func tasksFixture(t *testing.T) map[string]string {
	t.Helper()
	root := t.TempDir()
	proc := filepath.Join(root, "proc")
	sys := filepath.Join(root, "sys")
	slice := filepath.Join(root, "slice")
	write := func(path, body string) {
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, []byte(body), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	write(filepath.Join(proc, "stat"), "cpu 1000 0 500 8000 0 0 0 0\ncpu0 500 0 250 4000 0 0 0 0\ncpu1 500 0 250 4000 0 0 0 0\n")
	write(filepath.Join(proc, "meminfo"), "MemTotal: 16000000 kB\nMemAvailable: 8000000 kB\n")
	write(filepath.Join(proc, "net/dev"), "Inter-|   Receive                                                |  Transmit\n lo: 100 1 0 0 0 0 0 0 100 1 0 0 0 0 0 0\n eth0: 2048 10 0 0 0 0 0 0 1024 8 0 0 0 0 0 0\n")
	// Two processes we own, in one app scope.
	uid := os.Getuid()
	for _, p := range []struct {
		pid  int
		name string
	}{{424242, "demo-app"}, {424243, "demo-helper"}} {
		write(filepath.Join(proc, fmt.Sprint(p.pid), "status"), fmt.Sprintf("Name:\t%s\nUid:\t%d %d %d %d\nRssAnon: 40960 kB\n", p.name, uid, uid, uid, uid))
		write(filepath.Join(proc, fmt.Sprint(p.pid), "stat"), fmt.Sprintf("%d (%s) S 1 0 0 0 0 0 0 0 0 0 10 20 0 0 0 0 0 0 777 0 2", p.pid, p.name))
	}
	unit := "app-whiterose-com.example.Demo-100-0.scope"
	write(filepath.Join(slice, unit, "cgroup.procs"), "424242\n424243\n")
	write(filepath.Join(slice, unit, "memory.stat"), "anon 268435456\nfile 1000000\n")
	write(filepath.Join(slice, unit, "cpu.stat"), "usage_usec 5000000\n")
	write(filepath.Join(slice, unit, "io.stat"), "8:0 rbytes=1048576 wbytes=2097152\n")
	_ = sys
	return map[string]string{
		"WHITEROSE_TASKS_PROC":  proc,
		"WHITEROSE_TASKS_SYS":   sys,
		"WHITEROSE_TASKS_SLICE": slice,
		"WHITEROSE_APP_SLICE":   slice,
	}
}

func TestTasksAppsPerformanceAndProtection(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Env: tasksFixture(t)})
	s.Launch("whiterose-tasks", tasksBinary(s))
	app := s.WaitWindow("Task manager")

	// Apps & processes: the fixture's one app, selected, with its details.
	app.WaitFor(uitest.Named("Apps & processes").Role("radio button"))
	app.Click(uitest.Named("Search apps, processes or PID").Role("text"))
	app.Type("Demo")
	app.Activate(uitest.Named("Demo app").Role("list item"))
	app.Click(uitest.Named("Details").Role("push button"))
	app.WaitFor(uitest.Named("App details"))
	app.Click(uitest.Named("Done").Role("push button"))
	app.Gone(uitest.Named("App details"))

	// End and Force end are separate, confirmed actions; Cancel acts on neither.
	app.Click(uitest.Named("End").Role("push button"))
	app.WaitFor(uitest.Named("End this app?"))
	app.Click(uitest.Named("Cancel").Role("push button"))
	app.Gone(uitest.Named("End this app?"))
	app.Click(uitest.Named("Force end").Role("push button"))
	app.WaitFor(uitest.Named("Force end this app?"))
	app.Click(uitest.Named("Cancel").Role("push button"))
	app.Gone(uitest.Named("Force end this app?"))

	// Pause freezes updates; Resume brings them back.
	app.Click(uitest.Named("Pause").Role("push button"))
	app.WaitFor(uitest.Named("Resume").Role("push button"))
	app.Click(uitest.Named("Resume").Role("push button"))
	app.WaitFor(uitest.Named("Pause").Role("push button"))

	// Performance: every metric renders.
	app.Click(uitest.Named("Performance").Role("radio button"))
	app.WaitFor(uitest.Named("CPU").Role("radio button"))
	for _, metric := range []string{"Memory", "Storage", "Network", "GPU", "CPU"} {
		app.Click(uitest.Named(metric).Role("radio button"))
	}

	// Memory protection edits the same settings as Settings › Apps › Memory.
	app.Click(uitest.Named("Memory protection").Role("radio button"))
	app.WaitFor(uitest.Named("Close apps when memory runs out").Role("check box"))
	app.Click(uitest.Named("Close apps when memory runs out").Role("check box"))
	if !uitest.WaitUntil(func() bool { return s.Setting("memory.autoClose") == "false" }) {
		t.Fatal("memory.autoClose wasn't changed")
	}
	app.Click(uitest.Named("Keep Demo running").Role("check box"))
	if !uitest.WaitUntil(func() bool { return strings.Contains(s.Setting("memory.protected"), "com.example.Demo") }) {
		t.Fatal("memory.protected wasn't changed")
	}
}
