package main

import (
	"bufio"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"os/user"
	"path/filepath"
	"strconv"
	"sync"

	"whiterose/daemon/internal/apps"
)

// cmdAppsRoot is the root half of the app setup, run through pkexec:
//
//	whiterosed apps-root ID...
//
// It only takes ids from the catalogue; what each means (packages, vendor
// repositories, URLs) is fixed in internal/apps. Progress goes to stdout as
// one JSON event per line.
func cmdAppsRoot(args []string, stdout, stderr io.Writer) int {
	if os.Geteuid() != 0 {
		fmt.Fprintln(stderr, "whiterosed apps-root: run through pkexec (it installs system packages)")
		return 2
	}
	d, ok := apps.DetectDistro("/etc/os-release")
	if !ok {
		fmt.Fprintln(stderr, "whiterosed apps-root: this distribution isn't supported (Ubuntu or Fedora)")
		return 2
	}
	// Whoever pkexec ran this for gets zsh as their shell.
	name := ""
	if uid, err := strconv.Atoi(os.Getenv("PKEXEC_UID")); err == nil && uid > 0 {
		if u, err := user.LookupId(strconv.Itoa(uid)); err == nil {
			name = u.Username
		}
	}
	steps, err := apps.Plan(d, args, name)
	if err != nil {
		fmt.Fprintln(stderr, "whiterosed apps-root:", err)
		return 2
	}
	enc := json.NewEncoder(stdout)
	if err := apps.Run(context.Background(), d, steps, func(e apps.Event) { _ = enc.Encode(e) }); err != nil {
		return 1
	}
	return 0
}

// appSetup runs installs for the daemon, one at a time.
type appSetup struct {
	mu        sync.Mutex
	broadcast func(any)
	done      func()
}

func (s *appSetup) status() (any, error) {
	d, ok := apps.DetectDistro("/etc/os-release")
	if !ok {
		return nil, errors.New("this distribution isn't supported (Ubuntu or Fedora)")
	}
	installed := apps.Installed(d)
	type view struct {
		ID        string `json:"id"`
		Name      string `json:"name"`
		What      string `json:"what"`
		Source    string `json:"source"`
		Community bool   `json:"community"`
		Installed bool   `json:"installed"`
	}
	var list []view
	for _, a := range apps.Catalog {
		src := a.Sources[d]
		list = append(list, view{a.ID, a.Name, a.What, sourceName(src), src.Community, installed[a.ID]})
	}
	return map[string]any{"distro": string(d), "apps": list}, nil
}

func sourceName(s apps.Source) string {
	switch s.Kind {
	case apps.Native:
		return "distro"
	case apps.Vendor:
		return "vendor"
	case apps.Copr:
		return "copr"
	case apps.Flatpak:
		return "flathub"
	}
	return string(s.Kind)
}

// install runs the root half through pkexec (the shell's polkit prompt asks
// for the password), then your part: Oh My Zsh with your theme, and your VS
// Code extensions. Progress is broadcast as "apps" events.
func (s *appSetup) install(ids []string) (any, error) {
	if !s.mu.TryLock() {
		return nil, errors.New("apps are already being installed")
	}
	defer s.mu.Unlock()
	for _, id := range ids {
		if _, ok := apps.Find(id); !ok {
			return nil, fmt.Errorf("unknown app %q", id)
		}
	}
	emit := func(e apps.Event) {
		s.broadcast(map[string]any{"event": "apps", "step": e.Step, "apps": e.Apps, "state": e.State, "line": e.Line, "error": e.Error})
	}
	ctx := context.Background()
	var rootErr error
	if len(ids) > 0 {
		self, err := rootSelf()
		if err != nil {
			return nil, err
		}
		cmd := exec.CommandContext(ctx, "pkexec", append([]string{self, "apps-root"}, ids...)...)
		out, err := cmd.StdoutPipe()
		if err != nil {
			return nil, err
		}
		var stderr bytesTail
		cmd.Stderr = &stderr
		if err := cmd.Start(); err != nil {
			return nil, err
		}
		sc := bufio.NewScanner(out)
		for sc.Scan() {
			var e apps.Event
			if json.Unmarshal(sc.Bytes(), &e) == nil {
				emit(e)
			}
		}
		if err := cmd.Wait(); err != nil {
			var exit *exec.ExitError
			if errors.As(err, &exit) && (exit.ExitCode() == 126 || exit.ExitCode() == 127) {
				return nil, errors.New("not allowed (wrong password, or cancelled)")
			}
			rootErr = fmt.Errorf("some apps didn't install%s", stderr.suffix())
		}
	}

	// Your part, as you.
	profile := profileDir()
	home, _ := os.UserHomeDir()
	for _, id := range ids {
		var err error
		switch id {
		case "zsh":
			emit(apps.Event{Step: "Your zsh theme", Apps: []string{id}, State: "running"})
			err = apps.SetupZsh(ctx, home, profile, func(l string) {
				emit(apps.Event{Step: "Your zsh theme", Apps: []string{id}, State: "running", Line: l})
			})
			finish(emit, "Your zsh theme", id, err)
		case "vscode":
			if _, e := exec.LookPath("code"); e != nil {
				continue
			}
			emit(apps.Event{Step: "Your VS Code extensions", Apps: []string{id}, State: "running"})
			err = apps.VSCodeExtensions(ctx, profile, func(l string) {
				emit(apps.Event{Step: "Your VS Code extensions", Apps: []string{id}, State: "running", Line: l})
			})
			finish(emit, "Your VS Code extensions", id, err)
		}
		if err != nil && rootErr == nil {
			rootErr = err
		}
	}
	s.done()
	if rootErr != nil {
		return nil, rootErr
	}
	return s.status()
}

func finish(emit func(apps.Event), step, id string, err error) {
	if err != nil {
		emit(apps.Event{Step: step, Apps: []string{id}, State: "failed", Error: err.Error()})
	} else {
		emit(apps.Event{Step: step, Apps: []string{id}, State: "done"})
	}
}

// profileDir is where your theme and extension list are: installed under
// /usr/share/whiterose/profile, or the repo's profile/ when run from a build.
func profileDir() string {
	if d := os.Getenv("WHITEROSE_PROFILE_DIR"); d != "" {
		return d
	}
	if exe, err := os.Executable(); err == nil {
		dir := filepath.Dir(exe)
		for _, c := range []string{filepath.Join(dir, "..", "share", "whiterose", "profile"), filepath.Join(dir, "..", "profile")} {
			if _, err := os.Stat(filepath.Join(c, "vscode-extensions.txt")); err == nil {
				return c
			}
		}
	}
	return "/usr/share/whiterose/profile"
}

// bytesTail keeps the end of a stream (pkexec's complaints).
type bytesTail struct{ b []byte }

func (t *bytesTail) Write(p []byte) (int, error) {
	t.b = append(t.b, p...)
	if len(t.b) > 2048 {
		t.b = t.b[len(t.b)-2048:]
	}
	return len(p), nil
}

func (t *bytesTail) suffix() string {
	if len(t.b) == 0 {
		return ""
	}
	return ": " + string(t.b)
}
