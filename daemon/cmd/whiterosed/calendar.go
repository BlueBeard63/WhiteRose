package main

import (
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"net/http"
	"time"

	"whiterose/daemon/internal/calendar"
	"whiterose/daemon/internal/weather"
)

var httpClient = &http.Client{Timeout: 20 * time.Second}

// cmdCalendar: `calendar events --from 2026-10-26 --to 2026-12-07` prints the
// events in that range as JSON (from the cache: instant, works offline);
// `calendar refresh` downloads the linked calendars.
func cmdCalendar(args []string, stdout, stderr io.Writer) int {
	if len(args) == 0 {
		fmt.Fprintln(stderr, "usage: whiterosed calendar events --from DATE --to DATE | refresh")
		return 2
	}
	d := calendar.DefaultDirs()
	switch args[0] {
	case "events":
		fs := flag.NewFlagSet("calendar events", flag.ContinueOnError)
		fs.SetOutput(stderr)
		fromS := fs.String("from", "", "first day (YYYY-MM-DD)")
		toS := fs.String("to", "", "day after the last (YYYY-MM-DD)")
		if err := fs.Parse(args[1:]); err != nil {
			return 2
		}
		from, err1 := time.ParseInLocation("2006-01-02", *fromS, time.Local)
		to, err2 := time.ParseInLocation("2006-01-02", *toS, time.Local)
		if err1 != nil || err2 != nil || !to.After(from) {
			fmt.Fprintln(stderr, "whiterosed: --from and --to must be dates (YYYY-MM-DD), --to after --from")
			return 2
		}
		evs, err := calendar.Events(d, from, to, time.Local)
		if err != nil {
			fmt.Fprintln(stderr, "whiterosed:", err)
			return 1
		}
		_ = json.NewEncoder(stdout).Encode(evs)
		return 0
	case "refresh":
		changed, errs := calendar.Refresh(context.Background(), d, httpClient)
		for _, e := range errs {
			fmt.Fprintln(stderr, "whiterosed:", e)
		}
		fmt.Fprintf(stdout, "%d calendar(s) updated\n", changed)
		if len(errs) > 0 {
			return 1
		}
		return 0
	}
	fmt.Fprintf(stderr, "whiterosed: unknown calendar command %q\n", args[0])
	return 2
}

// cmdWeather fetches the current conditions for the place in weather.toml.
func cmdWeather(_ []string, stdout, stderr io.Writer) int {
	d := calendar.DefaultDirs()
	n, err := weather.Refresh(context.Background(), d.Config, d.Cache, httpClient)
	if errors.Is(err, weather.ErrNotConfigured) {
		fmt.Fprintln(stderr, `whiterosed: set your city in ~/.config/whiterose/weather.toml, e.g. city = "London"`)
		return 3
	}
	if err != nil {
		fmt.Fprintln(stderr, "whiterosed:", err)
		return 1
	}
	_ = json.NewEncoder(stdout).Encode(n)
	return 0
}

// weatherView is the cached reading for the place currently configured, or
// nil (not set up, or nothing fetched yet).
func weatherView(d calendar.Dirs) *weather.Now {
	cfg, err := weather.LoadConfig(d.Config)
	if err != nil {
		return nil
	}
	n, ok := weather.Cached(d.Cache)
	if !ok || n.City != cfg.City {
		return nil
	}
	return &n
}

// refreshLoop runs fn now and then every `every`, poking after each run.
func refreshLoop(every time.Duration, fn func(), poke func()) {
	for {
		fn()
		poke()
		time.Sleep(every)
	}
}

func startRefreshers(stderr io.Writer, poke func()) {
	d := calendar.DefaultDirs()
	go refreshLoop(15*time.Minute, func() {
		_, errs := calendar.Refresh(context.Background(), d, httpClient)
		for _, e := range errs {
			fmt.Fprintln(stderr, "whiterosed:", e)
		}
	}, poke)
	go refreshLoop(30*time.Minute, func() {
		if _, err := weather.Refresh(context.Background(), d.Config, d.Cache, httpClient); err != nil && !errors.Is(err, weather.ErrNotConfigured) {
			fmt.Fprintln(stderr, "whiterosed: weather:", err)
		}
	}, poke)
}
