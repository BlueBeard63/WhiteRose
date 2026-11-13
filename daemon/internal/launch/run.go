package launch

import (
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"os/signal"
	"path/filepath"
	"strings"
	"sync"
	"syscall"
	"time"
)

// RestartCode: the compositor exits with it to have the session started
// again (Updates › Restart session now). Matches compositor/src/keybind.rs.
const RestartCode = 42

// The pieces that report ready (compositor/src/ready.rs, whiterosed serve).
var readyMarks = []string{"compositor", "daemon", "shell"}

type Config struct {
	Root      string // the versions: <Root>/<version>/bin/...
	Home      string
	StateHome string // $XDG_STATE_HOME
	Runtime   string // $XDG_RUNTIME_DIR (the ready markers)
	// The session's command line for a version's folder.
	Command func(dir string) []string
	Env     []string

	Installed func(root string) []string
	Newer     func(a, b string) bool

	Ready   time.Duration // all pieces ready within this (20 s)
	Survive time.Duration // and still running at this (60 s)
	Stop    time.Duration // between asking to stop and making it (5 s)
	LogMax  int64         // the session log's size limit (0: LogMax)

	Logf func(format string, args ...any)
}

func (c Config) dir() string             { return filepath.Join(c.StateHome, "whiterose") }
func (c Config) snapshotDir() string     { return filepath.Join(c.dir(), "snapshot") }
func (c Config) LogFile(v string) string { return filepath.Join(c.dir(), "update-"+v+".log") }

func (c Config) save(path string, s State) {
	if err := Save(path, s); err != nil {
		c.logf("saving %s: %v", path, err)
	}
}

func (c Config) logf(format string, args ...any) {
	if c.Logf != nil {
		c.Logf(format, args...)
	}
}

// Run starts sessions until one ends other than by restarting, and returns
// its exit code.
func Run(c Config) (int, error) {
	stateFile := StateFile(c.StateHome)
	for {
		s := Load(stateFile)
		plan, s, err := Decide(s, c.Installed(c.Root), c.Newer)
		if err != nil {
			return 1, err
		}
		if plan.Trial {
			// A failed try puts these back. One snapshot only: it's replaced.
			if err := Snapshot(c.Home, c.snapshotDir()); err != nil {
				c.logf("settings snapshot: %v", err)
			} else {
				s.Snapshot = time.Now().Unix()
			}
		}
		c.save(stateFile, s)

		code, err := c.start(plan, stateFile)
		if err != nil && plan.Trial {
			// Didn't come up: the version before, now.
			c.logf("%s didn't start (%v); starting %s", plan.Version, err, plan.Fallback)
			code, err = c.start(Plan{Version: plan.Fallback}, stateFile)
		}
		if err != nil {
			return 1, err
		}
		if code != RestartCode {
			return code, nil
		}
		c.logf("restarting the session")
	}
}

// start runs one session of plan.Version; for a trial, it returns an error
// if the health check failed (it's been stopped, noted and the settings put
// back), else the session's exit code once it ends.
func (c Config) start(plan Plan, stateFile string) (int, error) {
	dir := filepath.Join(c.Root, plan.Version)
	ready, err := os.MkdirTemp(c.Runtime, "whiterose-ready-")
	if err != nil {
		return 1, err
	}
	defer os.RemoveAll(ready)

	argv := c.Command(dir)
	cmd := exec.Command(argv[0], argv[1:]...)
	cmd.Env = append(append([]string(nil), c.Env...),
		"WHITEROSE_DIR="+dir,
		"WHITEROSE_VERSION="+plan.Version,
		"WHITEROSE_READY_DIR="+ready,
		// (Updates › Restart session now works: something starts it again.)
		"WHITEROSE_LAUNCHER=1",
		"PATH="+filepath.Join(dir, "bin")+":"+envPath(c.Env),
	)
	cmd.Stdin, cmd.Stdout = os.Stdin, os.Stdout
	// Its own process group: stopping it stops the daemon and shell too.
	cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
	tail := &tailWriter{max: 200}
	out := []io.Writer{os.Stderr, tail}
	max := c.LogMax
	if max == 0 {
		max = LogMax
	}
	if log, err := openSessionLog(c.StateHome, plan.Version, max); err == nil {
		defer log.Close()
		out = append(out, log)
	} else {
		c.logf("session log: %v", err)
	}
	cmd.Stderr = io.MultiWriter(out...)
	c.logf("starting %s%s", plan.Version, map[bool]string{true: " (trying it)"}[plan.Trial])
	if err := cmd.Start(); err != nil {
		if plan.Trial {
			c.failed(plan, stateFile, "couldn't run: "+err.Error(), nil)
		}
		return 1, err
	}

	// Logging out signals us: pass it on.
	sigs := make(chan os.Signal, 1)
	signal.Notify(sigs, syscall.SIGTERM, syscall.SIGHUP, syscall.SIGINT)
	defer signal.Stop(sigs)
	done := make(chan error, 1)
	go func() { done <- cmd.Wait() }()

	if plan.Trial {
		switch r := c.watch(cmd, ready, done, sigs); {
		case r.reason != "":
			if !r.exited {
				c.stop(cmd, done)
			}
			c.failed(plan, stateFile, r.reason, tail.lines())
			return 1, errors.New(r.reason)
		case r.quit:
			// Logged out during the check: not the version's fault, and
			// not a try.
			s := Load(stateFile)
			if s.Pending == plan.Version && s.Attempts > 0 {
				s.Attempts--
				c.save(stateFile, s)
			}
			return exitCode(cmd), nil
		default:
			c.save(stateFile, Passed(Load(stateFile), plan.Version))
			c.logf("%s started fine", plan.Version)
			if r.exited {
				return exitCode(cmd), nil
			}
		}
	}
	for {
		select {
		case <-done:
			return exitCode(cmd), nil
		case sig := <-sigs:
			syscall.Kill(-cmd.Process.Pid, sig.(syscall.Signal))
		}
	}
}

type checked struct {
	reason string // why it failed ("" if it didn't)
	exited bool   // the session has ended (reaped)
	quit   bool   // ended because you logged out during the check
}

// watch runs the health check: every piece ready within c.Ready, and still
// running at c.Survive (or ended normally, once up: logged out, restarted).
func (c Config) watch(cmd *exec.Cmd, ready string, done chan error, sigs chan os.Signal) checked {
	tick := time.NewTicker(100 * time.Millisecond)
	defer tick.Stop()
	readyBy := time.After(c.Ready)
	survived := time.After(c.Survive)
	up, quitting := false, false
	for {
		select {
		case <-done:
			code := exitCode(cmd)
			switch {
			case quitting:
				return checked{exited: true, quit: true}
			case up && (code == 0 || code == RestartCode):
				return checked{exited: true}
			}
			return checked{reason: describeExit(cmd, up), exited: true}
		case sig := <-sigs:
			quitting = true
			syscall.Kill(-cmd.Process.Pid, sig.(syscall.Signal))
		case <-tick.C:
			up = up || allReady(ready)
		case <-readyBy:
			if up = up || allReady(ready); !up {
				return checked{reason: "didn't finish starting within " + c.Ready.String() +
					" (no word from: " + strings.Join(missing(ready), ", ") + ")"}
			}
		case <-survived:
			return checked{}
		}
	}
}

func allReady(dir string) bool { return len(missing(dir)) == 0 }

func missing(dir string) []string {
	var m []string
	for _, n := range readyMarks {
		if _, err := os.Stat(filepath.Join(dir, n)); err != nil {
			m = append(m, n)
		}
	}
	return m
}

// stop: asked first (the process group), then made.
func (c Config) stop(cmd *exec.Cmd, done chan error) {
	syscall.Kill(-cmd.Process.Pid, syscall.SIGTERM)
	select {
	case <-done:
	case <-time.After(c.Stop):
		syscall.Kill(-cmd.Process.Pid, syscall.SIGKILL)
		<-done
	}
}

// failed notes a failed try (for the "couldn't start" prompt), writes its
// log, and puts the settings back.
func (c Config) failed(plan Plan, stateFile, reason string, out []string) {
	s := FailedTry(Load(stateFile), plan.Version, reason)
	c.save(stateFile, s)
	var b strings.Builder
	fmt.Fprintf(&b, "WhiteRose %s couldn't start (try %d of %d), %s\n", plan.Version, s.Attempts, Tries, time.Now().Format(time.DateTime))
	fmt.Fprintf(&b, "Reason: %s\n", reason)
	fmt.Fprintf(&b, "Went back to: %s\n", plan.Fallback)
	if len(out) > 0 {
		b.WriteString("\nWhat it printed last:\n")
		for _, l := range out {
			b.WriteString(l)
			b.WriteByte('\n')
		}
	}
	if err := os.WriteFile(c.LogFile(plan.Version), []byte(b.String()), 0o600); err != nil {
		c.logf("writing the log: %v", err)
	}
	if s.Snapshot != 0 {
		if err := Restore(c.Home, c.snapshotDir()); err != nil {
			c.logf("putting the settings back: %v", err)
		}
	}
	c.logf("%s failed: %s", plan.Version, reason)
}

func exitCode(cmd *exec.Cmd) int {
	if cmd.ProcessState == nil {
		return 1
	}
	if ws, ok := cmd.ProcessState.Sys().(syscall.WaitStatus); ok && ws.Signaled() {
		return 128 + int(ws.Signal())
	}
	return cmd.ProcessState.ExitCode()
}

func describeExit(cmd *exec.Cmd, up bool) string {
	when := "during start"
	if up {
		when = "soon after starting"
	}
	ps := cmd.ProcessState
	if ws, ok := ps.Sys().(syscall.WaitStatus); ok && ws.Signaled() {
		return fmt.Sprintf("compositor exited %s (signal %d, %s)", when, int(ws.Signal()), ws.Signal())
	}
	return fmt.Sprintf("compositor exited %s (code %d)", when, ps.ExitCode())
}

func envPath(env []string) string {
	for _, e := range env {
		if v, ok := strings.CutPrefix(e, "PATH="); ok {
			return v
		}
	}
	return "/usr/local/bin:/usr/bin:/bin"
}

// tailWriter keeps the last `max` lines written.
type tailWriter struct {
	mu   sync.Mutex
	max  int
	buf  []string
	part string
}

func (t *tailWriter) Write(p []byte) (int, error) {
	t.mu.Lock()
	defer t.mu.Unlock()
	lines := strings.Split(t.part+string(p), "\n")
	// The last piece has no newline yet.
	t.part = lines[len(lines)-1]
	t.buf = append(t.buf, lines[:len(lines)-1]...)
	if len(t.buf) > t.max {
		t.buf = t.buf[len(t.buf)-t.max:]
	}
	return len(p), nil
}

func (t *tailWriter) lines() []string {
	t.mu.Lock()
	defer t.mu.Unlock()
	out := append([]string(nil), t.buf...)
	if t.part != "" {
		out = append(out, t.part)
	}
	return out
}
