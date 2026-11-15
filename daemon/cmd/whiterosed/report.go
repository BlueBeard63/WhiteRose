package main

// Crash records and snapshots for bug reports (docs/plans/bug-reports.md):
// whiterosed snapshots each new crash record (the launcher writes one for
// the compositor, the compositor's supervisor for the shell and for
// whiterosed itself), and the report flow lists and reads them.

import (
	"bufio"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"whiterose/daemon/internal/crash"
	"whiterose/daemon/internal/launch"
	"whiterose/daemon/internal/report"
	"whiterose/daemon/internal/server"
	"whiterose/daemon/internal/snapshot"
	"whiterose/daemon/internal/update"
)

type reports struct {
	state     string // $XDG_STATE_HOME
	version   string
	broadcast func(any)

	// Where reports go, and how this build is checked (set by newReports;
	// tests change them).
	src    update.Source
	public string
	kind   string
	dir    string // this version's folder ("" in a build being worked on)

	mu    sync.Mutex
	build *buildView // checked once per session (not when offline)
}

// buildView: is this the released build? Only a verified one reports to
// the repository; the others save the report to a file.
type buildView struct {
	State   string `json:"state"` // update.Build*
	Version string `json:"version"`
	Kind    string `json:"kind"`
	ID      string `json:"id"`
	Repo    string `json:"repo"`
	Reason  string `json:"reason"`
}

func newReports(broadcast func(any)) *reports {
	state := os.Getenv("XDG_STATE_HOME")
	if state == "" {
		home, _ := os.UserHomeDir()
		state = filepath.Join(home, ".local", "state")
	}
	r := &reports{state: state, version: runningVersion(), broadcast: broadcast, kind: packageKind()}
	repo, _ := os.ReadFile(releaseRepoFile)
	pub, _ := os.ReadFile(installedKeyFile)
	r.src = update.Source{Repo: strings.TrimSpace(string(repo)), API: os.Getenv("WHITEROSE_UPDATE_API")}
	r.public = strings.TrimSpace(string(pub))
	if r.version != "" {
		r.dir = os.Getenv("WHITEROSE_DIR")
		if r.dir == "" {
			r.dir = filepath.Join(update.Root, r.version)
		}
	}
	return r
}

// checkBuild: verified, modified, unlisted, development or unknown.
func (r *reports) checkBuild(ctx context.Context) buildView {
	r.mu.Lock()
	defer r.mu.Unlock()
	if r.build != nil {
		return *r.build
	}
	v := buildView{Version: r.version, Kind: r.kind, Repo: r.src.Repo}
	switch {
	case r.version == "" || r.dir == "":
		v.State, v.Reason = update.BuildDevelopment, "a build being worked on, not an installed version"
		return v
	case r.kind == "" || r.public == "" || r.src.Repo == "":
		v.State, v.Reason = update.BuildUnlisted, "this build has no release key or repository to check against"
		return v
	}
	id, err := update.BuildID(r.dir)
	if err != nil {
		v.State, v.Reason = update.BuildModified, err.Error()
		return v
	}
	v.ID = id
	state, err := r.src.VerifyBuild(ctx, r.version, r.kind, id, r.public)
	v.State = state
	if err != nil {
		v.Reason = err.Error()
	}
	if state != update.BuildUnknown {
		r.build = &v
	}
	return v
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
	Prompted  bool   `json:"prompted"` // the island has offered to report it
}

func (r *reports) draftFile() string {
	return filepath.Join(r.state, "whiterose", "report-draft.json")
}

// input gathers what a report is made from: the draft, its record and
// snapshot, and the build.
func (r *reports) input(raw json.RawMessage) (report.Input, error) {
	var d report.Draft
	if err := json.Unmarshal(raw, &d); err != nil {
		return report.Input{}, err
	}
	rec, err := crash.Load(r.state, d.Snapshot)
	if err != nil {
		return report.Input{}, errors.New("that snapshot is gone (only the last 5 are kept): pick another")
	}
	in := report.Input{Draft: d, Record: rec, Now: time.Now(), MaxLines: 400}
	if b, err := os.ReadFile(crash.File(r.state, rec.ID, "snapshot.json")); err == nil {
		in.Snap = report.Load(b)
	}
	if b, err := os.ReadFile(crash.File(r.state, rec.ID, "output.txt")); err == nil {
		in.Output = string(b)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
	defer cancel()
	bv := r.checkBuild(ctx)
	in.Build, in.BuildID, in.Repo = bv.State, bv.ID, bv.Repo
	return in, nil
}

func (r *reports) list() []snapshotView {
	out := []snapshotView{}
	for _, rec := range crash.List(r.state) {
		_, err := os.Stat(crash.File(r.state, rec.ID, "snapshot.json"))
		_, errOut := os.Stat(crash.File(r.state, rec.ID, "output.txt"))
		_, errAsked := os.Stat(crash.File(r.state, rec.ID, "prompted"))
		out = append(out, snapshotView{ID: rec.ID, What: rec.What, Title: rec.Describe(), Time: rec.Time,
			Ready: err == nil, Version: rec.Version, HasOutput: errOut == nil, Prompted: errAsked == nil})
	}
	return out
}

func (r *reports) ops() map[string]func(server.Request) (any, error) {
	return map[string]func(server.Request) (any, error){
		"report_snapshots": func(server.Request) (any, error) { return r.list(), nil },
		// The island offered to report it ("Report this" after a crash):
		// not again, even after the shell restarts.
		"report_prompted": func(req server.Request) (any, error) {
			rec, err := crash.Load(r.state, req.Name)
			if err != nil {
				return nil, err
			}
			return nil, os.WriteFile(crash.File(r.state, rec.ID, "prompted"), nil, 0o600)
		},
		// Open the report flow in this session's island (Settings › About,
		// which runs apart from the shell); Name: a record to start on.
		"report_show": func(req server.Request) (any, error) {
			if r.broadcast != nil {
				r.broadcast(map[string]any{"event": "report_show", "from": req.Name})
			}
			return nil, nil
		},
		// The report flow's draft (kept here so a shell restart keeps it).
		"report_draft": func(server.Request) (any, error) {
			b, err := os.ReadFile(r.draftFile())
			if err != nil {
				return nil, nil
			}
			return json.RawMessage(b), nil
		},
		"report_draft_save": func(req server.Request) (any, error) {
			var d report.Draft
			if err := json.Unmarshal(req.Value, &d); err != nil {
				return nil, err
			}
			b, _ := json.Marshal(d)
			return nil, os.WriteFile(r.draftFile(), b, 0o600)
		},
		"report_draft_clear": func(server.Request) (any, error) {
			os.Remove(r.draftFile())
			return nil, nil
		},
		// The last step: each section's exact text.
		"report_preview": func(req server.Request) (any, error) {
			in, err := r.input(req.Value)
			if err != nil {
				return nil, err
			}
			out := []map[string]string{}
			for _, s := range report.Sections {
				if t := report.Section(in, s); t != "" {
					out = append(out, map[string]string{"key": s, "text": t})
				}
			}
			return out, nil
		},
		// Open report: the file in Downloads, and the prefilled form (only
		// for a verified build); the draft is done with.
		"report_open": func(req server.Request) (any, error) {
			in, err := r.input(req.Value)
			if err != nil {
				return nil, err
			}
			home, _ := os.UserHomeDir()
			dir := report.DownloadsDir(home)
			if err := os.MkdirAll(dir, 0o755); err != nil {
				return nil, err
			}
			name := "whiterose-report-" + in.Now.Format("20060102-150405") + ".txt"
			path := filepath.Join(dir, name)
			if err := os.WriteFile(path, []byte(report.File(in)), 0o600); err != nil {
				return nil, err
			}
			out := map[string]string{"file": path, "build": in.Build, "url": ""}
			if in.Build == update.BuildVerified {
				out["url"] = report.FormURL(in, name)
			}
			os.Remove(r.draftFile())
			return out, nil
		},
		// Whether a report may go to the repository (only a verified build's).
		"report_build": func(server.Request) (any, error) {
			ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
			defer cancel()
			return r.checkBuild(ctx), nil
		},
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
