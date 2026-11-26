package main

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"strings"
	"time"

	"whiterose/daemon/internal/clocks"
	"whiterose/daemon/internal/server"
	"whiterose/daemon/internal/weather"
)

func startClocks(srv *server.Server, done <-chan struct{}, log io.Writer) {
	updates := make(chan clocks.State, 1)
	loc := time.Local
	if _, overridden := os.LookupEnv("TZ"); !overridden {
		if loaded, _, err := clockLocation("/etc/localtime"); err == nil {
			loc = loaded
		}
	}
	service, err := clocks.New(clocks.Path(), loc, clocks.Now, func(state clocks.State) {
		srv.SetTopic("clocks", state)
		select {
		case updates <- state:
		default:
		}
	})
	if err != nil {
		fmt.Fprintln(log, "whiterosed: clocks:", err)
		for _, op := range []string{"clocks_state", "clocks_command", "clocks_cities"} {
			srv.Ops[op] = func(server.Request) (any, error) { return nil, err }
		}
		return
	}
	srv.Ops["clocks_state"] = func(server.Request) (any, error) { return service.Snapshot(), nil }
	srv.Ops["clocks_command"] = func(r server.Request) (any, error) {
		var c clocks.Command
		if err := json.Unmarshal(r.Args, &c); err != nil {
			return nil, err
		}
		return service.Apply(c)
	}
	srv.Ops["clocks_open"] = func(r server.Request) (any, error) {
		var x struct {
			View string `json:"view"`
		}
		if err := json.Unmarshal(r.Args, &x); err != nil {
			return nil, err
		}
		switch x.View {
		case "timers", "stopwatch", "alarms", "world":
		default:
			x.View = "timers"
		}
		srv.Broadcast(map[string]any{"event": "clocks_open", "view": x.View})
		return nil, nil
	}
	srv.Ops["clocks_cities"] = func(r server.Request) (any, error) {
		var x struct {
			Query string `json:"query"`
		}
		if err := json.Unmarshal(r.Args, &x); err != nil {
			return nil, err
		}
		x.Query = strings.TrimSpace(x.Query)
		if len(x.Query) < 2 || len(x.Query) > 200 {
			return []clocks.City{}, nil
		}
		offline := clocks.SearchZones(x.Query)
		if strings.Contains(x.Query, "/") || x.Query == "UTC" {
			return offline, nil
		}
		ctx, cancel := context.WithTimeout(context.Background(), 8*time.Second)
		defer cancel()
		cities, err := weather.SearchCities(ctx, httpClient, x.Query, 12)
		if err != nil {
			if len(offline) > 0 {
				return offline, nil
			}
			return nil, err
		}
		out := []clocks.City{}
		seen := map[string]bool{}
		for _, city := range cities {
			if _, e := time.LoadLocation(city.Timezone); e != nil || city.Timezone == "" || seen[city.Timezone] {
				continue
			}
			seen[city.Timezone] = true
			out = append(out, clocks.City{Name: city.Name, Country: city.Country, Zone: city.Timezone})
		}
		if len(out) == 0 {
			return offline, nil
		}
		return out, nil
	}
	srv.SetTopic("clocks", service.Snapshot())
	go service.Run(done, func(err error) { fmt.Fprintln(log, "whiterosed: clocks:", err) })
	go watchClockLocation(service, done, func(err error) { fmt.Fprintln(log, "whiterosed: clocks:", err) })
	go clockNotifications(service, updates, done, log)
}
