package main

import (
	"errors"
	"fmt"
	"io"
	"io/fs"
	"os"
	"strconv"

	"whiterose/daemon/internal/backlight"
	"whiterose/daemon/internal/rfkill"
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
	// Changes whenever a calendar was downloaded or a calendar file changed:
	// the shell re-reads events.
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
