package main

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"time"

	"whiterose/daemon/internal/clocks"
)

// Go caches time.Local once. Reload the tzfile when timedated changes it,
// including systems that copy /etc/localtime rather than symlinking it.
func clockLocation(path string) (*time.Location, []byte, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, nil, err
	}
	name := "Local time"
	if resolved, err := filepath.EvalSymlinks(path); err == nil {
		if _, zone, found := strings.Cut(resolved, "/zoneinfo/"); found {
			name = zone
		}
	}
	loc, err := time.LoadLocationFromTZData(name, data)
	return loc, data, err
}

func watchClockLocation(s *clocks.Service, done <-chan struct{}, report func(error)) {
	// A process-specific TZ is an explicit override of the system timezone.
	if _, overridden := os.LookupEnv("TZ"); overridden {
		return
	}
	_, previous, _ := clockLocation("/etc/localtime")
	tick := time.NewTicker(30 * time.Second)
	defer tick.Stop()
	for {
		select {
		case <-done:
			return
		case <-tick.C:
			loc, data, err := clockLocation("/etc/localtime")
			if err == nil && !bytes.Equal(data, previous) {
				previous = data
				if err := s.SetLocation(loc); err != nil {
					report(err)
				}
			}
			// Rebase running readouts after wall-clock changes even if only
			// world clocks (and no active countdown) were open.
			if err := s.Tick(); err != nil {
				report(err)
			}
		}
	}
}
