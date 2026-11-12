package main

import (
	"fmt"
	"io"
	"os"
	"os/exec"
	"os/signal"
	"syscall"

	"github.com/godbus/dbus/v5"

	"whiterose/daemon/internal/portal"
)

// cmdPortal serves WhiteRose's xdg-desktop-portal backend (the
// FileChooser: Files as every app's Open and Save dialog) on the session
// bus until it's told to stop. D-Bus starts it when the portal first asks.
func cmdPortal(_ []string, _, stderr io.Writer) int {
	files := os.Getenv("WHITEROSE_FILES")
	if files == "" {
		p, err := exec.LookPath("whiterose-files")
		if err != nil {
			fmt.Fprintln(stderr, "whiterosed portal: whiterose-files isn't installed")
			return 1
		}
		files = p
	}
	conn, err := dbus.ConnectSessionBus()
	if err != nil {
		fmt.Fprintln(stderr, "whiterosed portal:", err)
		return 1
	}
	defer conn.Close()
	fc := &portal.FileChooser{Files: files, Log: func(f string, a ...any) { fmt.Fprintf(stderr, "whiterosed portal: "+f+"\n", a...) }}
	if err := portal.Serve(conn, fc); err != nil {
		fmt.Fprintln(stderr, "whiterosed portal:", err)
		return 1
	}
	sigs := make(chan os.Signal, 1)
	signal.Notify(sigs, syscall.SIGTERM, syscall.SIGINT)
	<-sigs
	return 0
}
