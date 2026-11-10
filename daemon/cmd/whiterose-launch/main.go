// Command whiterose-launch starts the WhiteRose session (whiterose-session
// runs it): on the version that works, or trying a newly installed one with
// a health check and going back if it doesn't come up. See internal/launch.
//
//	whiterose-launch [--backend drm|winit]
//
// WHITEROSE_DIR set (a build being worked on) runs that, no checks.
// WHITEROSE_ROOT changes where the versions are (/usr/lib/whiterose).
package main

import (
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"syscall"
	"time"

	"whiterose/daemon/internal/launch"
	"whiterose/daemon/internal/update"
)

func main() {
	backend := flag.String("backend", "drm", "the compositor's backend")
	flag.Parse()

	command := func(dir string) []string {
		return []string{
			filepath.Join(dir, "bin", "whiterose-wm"), "--backend", *backend,
			"--daemon", "whiterosed serve",
			"--shell", "qs -p " + filepath.Join(dir, "shell"),
			// Apps started by D-Bus and systemd (portals, the file chooser)
			// need to find this session's display.
			"-c", "dbus-update-activation-environment --systemd WAYLAND_DISPLAY XDG_CURRENT_DESKTOP XDG_SESSION_TYPE WHITEROSE_DIR",
		}
	}
	logf := func(format string, args ...any) {
		fmt.Fprintf(os.Stderr, "whiterose-launch: "+format+"\n", args...)
	}

	if dir := os.Getenv("WHITEROSE_DIR"); dir != "" {
		argv := command(dir)
		os.Setenv("PATH", filepath.Join(dir, "bin")+":"+os.Getenv("PATH"))
		err := syscall.Exec(argv[0], argv, os.Environ())
		logf("%v", err)
		os.Exit(1)
	}

	home, err := os.UserHomeDir()
	if err != nil {
		logf("%v", err)
		os.Exit(1)
	}
	state := os.Getenv("XDG_STATE_HOME")
	if state == "" {
		state = filepath.Join(home, ".local", "state")
	}
	runtime := os.Getenv("XDG_RUNTIME_DIR")
	if runtime == "" {
		runtime = os.TempDir()
	}
	root := os.Getenv("WHITEROSE_ROOT")
	if root == "" {
		root = update.Root
	}
	code, err := launch.Run(launch.Config{
		Root: root, Home: home, StateHome: state, Runtime: runtime,
		Command: command, Env: os.Environ(),
		Installed: update.Installed, Newer: update.Newer,
		Ready: 20 * time.Second, Survive: 60 * time.Second, Stop: 5 * time.Second,
		Logf: logf,
	})
	if err != nil {
		logf("%v", err)
	}
	os.Exit(code)
}
