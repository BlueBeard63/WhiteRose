package launch

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"whiterose/daemon/internal/update"
)

func TestStateFile(t *testing.T) {
	p := filepath.Join(t.TempDir(), "whiterose", "version")
	if s := Load(p); s != (State{}) {
		t.Fatalf("no file: %+v", s)
	}
	in := State{Current: "0.4.2", Pending: "0.5.0", Attempts: 1, Failed: "0.5.0", Reason: "crashed\nbadly", Snapshot: 99}
	if err := Save(p, in); err != nil {
		t.Fatal(err)
	}
	out := Load(p)
	in.Reason = "crashed badly"
	if out != in {
		t.Errorf("%+v != %+v", out, in)
	}
	if fi, _ := os.Stat(p); fi.Mode().Perm() != 0o600 {
		t.Errorf("mode %v", fi.Mode())
	}
}

func TestDecide(t *testing.T) {
	newer := update.Newer
	both := []string{"0.5.0", "0.4.2"}

	// First run: nothing known, the newest.
	p, s, _ := Decide(State{}, both, newer)
	if p.Version != "0.5.0" || p.Trial || s.Current != "0.5.0" {
		t.Errorf("first run: %+v %+v", p, s)
	}

	// A newer one installed: tried, twice at most, the working one behind it.
	s = State{Current: "0.4.2"}
	for try := 1; try <= Tries; try++ {
		p, s, _ = Decide(s, both, newer)
		if !p.Trial || p.Version != "0.5.0" || p.Fallback != "0.4.2" || s.Attempts != try {
			t.Fatalf("try %d: %+v %+v", try, p, s)
		}
		s = FailedTry(s, "0.5.0", "crashed")
	}
	p, s, _ = Decide(s, both, newer)
	if p.Trial || p.Version != "0.4.2" || s.Failed != "0.5.0" {
		t.Errorf("after two failures: %+v %+v", p, s)
	}

	// Try again (the prompt): two more.
	p, s2, _ := Decide(TryAgain(s), both, newer)
	if !p.Trial || p.Version != "0.5.0" || s2.Attempts != 1 || s2.Failed != "" {
		t.Errorf("try again: %+v %+v", p, s2)
	}
	// Stay: not tried again...
	stay := Stay(s)
	p, stay, _ = Decide(stay, both, newer)
	if p.Trial || p.Version != "0.4.2" {
		t.Errorf("stay: %+v %+v", p, stay)
	}
	// ...until a newer one comes.
	p, _, _ = Decide(stay, append(both, "0.5.1"), newer)
	if !p.Trial || p.Version != "0.5.1" {
		t.Errorf("newer after staying: %+v", p)
	}

	// Passing makes it the one.
	s = Passed(State{Current: "0.4.2", Pending: "0.5.0", Attempts: 1}, "0.5.0")
	if s.Current != "0.5.0" || s.Pending != "" || s.Attempts != 0 {
		t.Errorf("passed: %+v", s)
	}

	// The working version removed (by hand): the newest that hasn't failed.
	p, s, _ = Decide(State{Current: "0.3.0", Failed: "0.5.0"}, both, newer)
	if p.Version != "0.4.2" || p.Trial || s.Current != "0.4.2" {
		t.Errorf("current gone: %+v %+v", p, s)
	}
	// The pending one removed: forgotten.
	p, s, _ = Decide(State{Current: "0.4.2", Pending: "0.6.0", Attempts: 1}, both, newer)
	if p.Version != "0.5.0" || !p.Trial || s.Pending != "0.5.0" {
		t.Errorf("pending gone: %+v %+v", p, s)
	}
	// An older one is never "pending".
	p, _, _ = Decide(State{Current: "0.5.0"}, both, newer)
	if p.Trial || p.Version != "0.5.0" {
		t.Errorf("older: %+v", p)
	}
	if _, _, err := Decide(State{}, nil, newer); err == nil {
		t.Error("nothing installed, yet decided")
	}
}

func TestSnapshotRestore(t *testing.T) {
	home := t.TempDir()
	snap := filepath.Join(t.TempDir(), "snapshot")
	cfg := filepath.Join(home, ".config/whiterose")
	db := filepath.Join(home, ".local/share/whiterose/settings.db")
	os.MkdirAll(filepath.Join(cfg, "sub"), 0o700)
	os.MkdirAll(filepath.Dir(db), 0o700)
	os.WriteFile(filepath.Join(cfg, "sub/a.conf"), []byte("old"), 0o600)
	os.Symlink("sub/a.conf", filepath.Join(cfg, "link"))
	os.WriteFile(db, []byte("db v1"), 0o600)

	if err := Snapshot(home, snap); err != nil {
		t.Fatal(err)
	}
	// The new version migrates things.
	os.WriteFile(filepath.Join(cfg, "sub/a.conf"), []byte("new"), 0o600)
	os.WriteFile(filepath.Join(cfg, "added"), []byte("x"), 0o600)
	os.WriteFile(db, []byte("db v2"), 0o600)
	os.WriteFile(db+"-wal", []byte("wal"), 0o600)

	if err := Restore(home, snap); err != nil {
		t.Fatal(err)
	}
	read := func(p string) string { b, _ := os.ReadFile(p); return string(b) }
	if read(filepath.Join(cfg, "sub/a.conf")) != "old" || read(db) != "db v1" {
		t.Error("not put back")
	}
	if _, err := os.Stat(filepath.Join(cfg, "added")); err == nil {
		t.Error("a file made since is still there")
	}
	if _, err := os.Stat(db + "-wal"); err == nil {
		t.Error("a database journal made since is still there")
	}
	if l, _ := os.Readlink(filepath.Join(cfg, "link")); l != "sub/a.conf" {
		t.Errorf("link %q", l)
	}
	// Taken again: replaced, one only.
	if err := Snapshot(home, snap); err != nil {
		t.Fatal(err)
	}
	if left, _ := os.ReadDir(filepath.Dir(snap)); len(left) != 1 {
		t.Errorf("snapshots: %v", left)
	}
	if Restore(home, filepath.Join(t.TempDir(), "none")) == nil {
		t.Error("restored nothing")
	}
}

// A fake version: bin/whiterose-wm is a shell script.
func version(t *testing.T, root, v, script string) {
	t.Helper()
	bin := filepath.Join(root, v, "bin")
	os.MkdirAll(bin, 0o755)
	if err := os.WriteFile(filepath.Join(bin, "whiterose-wm"), []byte("#!/bin/sh\n"+script), 0o755); err != nil {
		t.Fatal(err)
	}
}

const (
	comesUp = `for m in compositor daemon shell; do : > "$WHITEROSE_READY_DIR/$m"; done
`
	good    = comesUp + "sleep 0.5\nexit 0\n"
	crashes = "echo 'thread main panicked: no GPU' >&2\nkill -SEGV $$\n"
	hangs   = ": > \"$WHITEROSE_READY_DIR/compositor\"\nexec sleep 30\n"
)

func setup(t *testing.T) Config {
	t.Helper()
	root, home, state := t.TempDir(), t.TempDir(), t.TempDir()
	os.MkdirAll(filepath.Join(home, ".config/whiterose"), 0o700)
	os.WriteFile(filepath.Join(home, ".config/whiterose/look.conf"), []byte("light"), 0o600)
	return Config{
		Root: root, Home: home, StateHome: state, Runtime: t.TempDir(),
		Command:   func(dir string) []string { return []string{filepath.Join(dir, "bin/whiterose-wm")} },
		Env:       []string{"PATH=" + os.Getenv("PATH"), "HOME=" + home},
		Installed: update.Installed, Newer: update.Newer,
		Ready: 300 * time.Millisecond, Survive: 400 * time.Millisecond, Stop: time.Second,
		Logf: t.Logf,
	}
}

func TestRunNewVersionPasses(t *testing.T) {
	c := setup(t)
	version(t, c.Root, "0.4.2", good)
	version(t, c.Root, "0.5.0", good)
	Save(StateFile(c.StateHome), State{Current: "0.4.2"})
	if code, err := Run(c); err != nil || code != 0 {
		t.Fatalf("%d %v", code, err)
	}
	if s := Load(StateFile(c.StateHome)); s.Current != "0.5.0" || s.Pending != "" || s.Snapshot == 0 {
		t.Errorf("%+v", s)
	}
}

func TestRunFailsBack(t *testing.T) {
	for name, script := range map[string]string{"crashes": crashes, "hangs": hangs} {
		t.Run(name, func(t *testing.T) {
			c := setup(t)
			// The old version notes it ran (and with which settings).
			version(t, c.Root, "0.4.2", comesUp+`cat "$HOME/.config/whiterose/look.conf" > "$HOME/ran-0.4.2"`+"\nexit 0\n")
			// The new one changes a setting, then fails.
			version(t, c.Root, "0.5.0", `echo dark > "$HOME/.config/whiterose/look.conf"`+"\n"+script)
			Save(StateFile(c.StateHome), State{Current: "0.4.2"})

			start := time.Now()
			if code, err := Run(c); err != nil || code != 0 {
				t.Fatalf("%d %v", code, err)
			}
			if b, _ := os.ReadFile(filepath.Join(c.Home, "ran-0.4.2")); string(b) != "light\n" && string(b) != "light" {
				t.Errorf("the old version didn't run with the old settings: %q", b)
			}
			s := Load(StateFile(c.StateHome))
			if s.Current != "0.4.2" || s.Pending != "0.5.0" || s.Attempts != 1 || s.Failed != "0.5.0" {
				t.Errorf("%+v", s)
			}
			log, _ := os.ReadFile(c.LogFile("0.5.0"))
			want := map[string]string{"crashes": "signal 11", "hangs": "no word from: daemon, shell"}[name]
			if !strings.Contains(s.Reason, want) || !strings.Contains(string(log), want) {
				t.Errorf("reason %q, log:\n%s", s.Reason, log)
			}
			if name == "crashes" && !strings.Contains(string(log), "no GPU") {
				t.Errorf("its output isn't in the log:\n%s", log)
			}
			if name == "hangs" && time.Since(start) > 5*time.Second {
				t.Error("a hung version wasn't stopped")
			}
			// Second failure, then left alone.
			Run(c)
			Run(c)
			if s := Load(StateFile(c.StateHome)); s.Attempts != Tries || s.Current != "0.4.2" {
				t.Errorf("%+v", s)
			}
		})
	}
}

func TestRunRestarts(t *testing.T) {
	c := setup(t)
	// Restarts once (Updates › Restart session now), then logs out.
	version(t, c.Root, "0.5.0", comesUp+`n=$(cat "$HOME/n" 2>/dev/null || echo 0); echo $((n+1)) > "$HOME/n"
[ "$n" = 0 ] && exit 42
exit 3
`)
	if code, err := Run(c); err != nil || code != 3 {
		t.Fatalf("%d %v", code, err)
	}
	if b, _ := os.ReadFile(filepath.Join(c.Home, "n")); strings.TrimSpace(string(b)) != "2" {
		t.Errorf("ran %s times", b)
	}
}
