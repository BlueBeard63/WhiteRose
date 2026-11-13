package main

// Crash records and snapshots for bug reports (docs/plans/bug-reports.md):
// whiterosed snapshots each new crash record (the launcher writes one for
// the compositor, the compositor's supervisor for the shell and for
// whiterosed itself), and the report flow lists and reads them.

import (
	"bufio"
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"os"
	"path/filepath"
	"time"

	"whiterose/daemon/internal/crash"
	"whiterose/daemon/internal/launch"
	"whiterose/daemon/internal/server"
	"whiterose/daemon/internal/snapshot"
)

type reports struct {
	state     string // $XDG_STATE_HOME
	version   string
	broadcast func(any)
}

func newReports(broadcast func(any)) *reports {
	state := os.Getenv("XDG_STATE_HOME")
	if state == "" {
		home, _ := os.UserHomeDir()
		state = filepath.Join(home, ".local", "state")
	}
	return &reports{state: state, version: runningVersion(), broadcast: broadcast}
}

func (r *reports) sources() snapshot.Sources {
	return snapshot.Sources{
		SessionLog: launch.SessionLog(r.state),
		Version:    r.version,
		Compositor: compositorState,
	}
}

// compositorState: the windows and outputs events the compositor sends
// any client that connects (nil, nil outside a session).
func compositorState() (windows, outputs json.RawMessage) {
	sock := os.Getenv("WHITEROSE_SOCKET")
	if sock == "" {
		return nil, nil
	}
	c, err := net.DialTimeout("unix", sock, time.Second)
	if err != nil {
		return nil, nil
	}
	defer c.Close()
	c.SetReadDeadline(time.Now().Add(2 * time.Second))
	sc := bufio.NewScanner(c)
	sc.Buffer(make([]byte, 64<<10), 8<<20)
	for sc.Scan() && (windows == nil || outputs == nil) {
		var ev struct {
			Event string `json:"event"`
		}
		if json.Unmarshal(sc.Bytes(), &ev) != nil {
			continue
		}
		line := json.RawMessage(append([]byte(nil), sc.Bytes()...))
		switch ev.Event {
		case "windows":
			windows = line
		case "outputs":
			outputs = line
		}
	}
	return windows, outputs
}

// take writes record id's snapshot. A compositor crash is snapshotted at
// the next login, and anything noticed long after: those are late.
func (r *reports) take(rec crash.Record) error {
	late := rec.What == crash.Compositor || time.Since(time.UnixMilli(rec.Time)) > 2*time.Minute
	src := r.sources()
	if rec.What == crash.Compositor {
		// The crashed session's log is the one before this.
		src.SessionLog = launch.SessionLogBefore(r.state)
	}
	snap := snapshot.Take(src, late)
	b, err := json.Marshal(snap)
	if err != nil {
		return err
	}
	path := crash.File(r.state, rec.ID, "snapshot.json")
	tmp := path + ".tmp"
	if err := os.WriteFile(tmp, b, 0o600); err != nil {
		return err
	}
	return os.Rename(tmp, path)
}

// watch snapshots records that don't have one yet (looked at every few
// seconds: records are rare, and ReadDir of a 5-entry folder is cheap).
func (r *reports) watch() {
	for {
		changed := false
		for _, rec := range crash.List(r.state) {
			if _, err := os.Stat(crash.File(r.state, rec.ID, "snapshot.json")); err == nil {
				continue
			}
			if err := r.take(rec); err != nil {
				fmt.Fprintf(os.Stderr, "whiterosed: snapshot of %s: %v\n", rec.ID, err)
				continue
			}
			changed = true
		}
		if changed && r.broadcast != nil {
			r.broadcast(map[string]any{"event": "report_snapshots", "snapshots": r.list()})
		}
		time.Sleep(3 * time.Second)
	}
}

type snapshotView struct {
	ID        string `json:"id"`
	What      string `json:"what"`
	Title     string `json:"title"` // "The shell crashed (signal 11, SIGSEGV)"
	Time      int64  `json:"time"`  // unix ms
	Ready     bool   `json:"ready"` // its snapshot is taken
	Version   string `json:"version"`
	HasOutput bool   `json:"has_output"`
}

func (r *reports) list() []snapshotView {
	out := []snapshotView{}
	for _, rec := range crash.List(r.state) {
		_, err := os.Stat(crash.File(r.state, rec.ID, "snapshot.json"))
		_, errOut := os.Stat(crash.File(r.state, rec.ID, "output.txt"))
		out = append(out, snapshotView{ID: rec.ID, What: rec.What, Title: rec.Describe(), Time: rec.Time,
			Ready: err == nil, Version: rec.Version, HasOutput: errOut == nil})
	}
	return out
}

func (r *reports) ops() map[string]func(server.Request) (any, error) {
	return map[string]func(server.Request) (any, error){
		"report_snapshots": func(server.Request) (any, error) { return r.list(), nil },
		// SUPER+SHIFT+B, or "something that's happening now" in the flow.
		"report_snapshot_now": func(server.Request) (any, error) {
			id, err := crash.Write(r.state, crash.Record{What: crash.Manual, Version: r.version}, nil)
			if err != nil {
				return nil, err
			}
			rec, err := crash.Load(r.state, id)
			if err != nil {
				return nil, err
			}
			if err := r.take(rec); err != nil {
				return nil, err
			}
			return map[string]string{"id": id}, nil
		},
		// One record: its snapshot, and what it printed last (if kept).
		"report_snapshot": func(req server.Request) (any, error) {
			rec, err := crash.Load(r.state, req.Name)
			if err != nil {
				return nil, errors.New("that snapshot is gone (only the last 5 are kept)")
			}
			var snap json.RawMessage
			if b, err := os.ReadFile(crash.File(r.state, rec.ID, "snapshot.json")); err == nil {
				snap = b
			}
			output := ""
			if b, err := os.ReadFile(crash.File(r.state, rec.ID, "output.txt")); err == nil {
				output = string(b)
			}
			return map[string]any{"record": rec, "title": rec.Describe(), "snapshot": snap, "output": output}, nil
		},
	}
}
