package main

import (
	"encoding/json"
	"os"
	"strings"
	"testing"

	"whiterose/daemon/internal/crash"
	"whiterose/daemon/internal/launch"
	"whiterose/daemon/internal/server"
)

func TestReports(t *testing.T) {
	state := t.TempDir()
	t.Setenv("XDG_STATE_HOME", state)
	t.Setenv("WHITEROSE_SOCKET", "")
	r := newReports(nil)
	os.MkdirAll(state+"/whiterose", 0o700)
	os.WriteFile(launch.SessionLogBefore(state), []byte("WhiteRose 0.3.0, session started 2026-11-13 12:00:00\npanicked at cursor.rs\n"), 0o600)

	// A compositor crash (from the launcher): snapshotted late, from the
	// crashed session's log.
	id, err := crash.Write(state, crash.Record{What: crash.Compositor, Code: 101, Version: "0.3.0"}, []string{"panicked at cursor.rs"})
	if err != nil {
		t.Fatal(err)
	}
	rec, _ := crash.Load(state, id)
	if err := r.take(rec); err != nil {
		t.Fatal(err)
	}
	ops := r.ops()
	v, err := ops["report_snapshot"](server.Request{Name: id})
	if err != nil {
		t.Fatal(err)
	}
	b, _ := json.Marshal(v)
	for _, want := range []string{`"late":true`, `panicked at cursor.rs`, `The compositor exited with code 101`} {
		if !strings.Contains(string(b), want) {
			t.Errorf("no %s in %s", want, b)
		}
	}
	if _, err := ops["report_snapshot"](server.Request{Name: "../../x"}); err == nil {
		t.Error("read a path")
	}

	// Taken now: not late, with the load.
	v, err = ops["report_snapshot_now"](server.Request{})
	if err != nil {
		t.Fatal(err)
	}
	list, _ := ops["report_snapshots"](server.Request{})
	views := list.([]snapshotView)
	if len(views) != 2 || views[0].What != crash.Manual || !views[0].Ready || views[1].ID != id || !views[1].HasOutput {
		t.Errorf("%+v", views)
	}
	b, _ = os.ReadFile(crash.File(state, views[0].ID, "snapshot.json"))
	if strings.Contains(string(b), `"late"`) || !strings.Contains(string(b), `"load"`) {
		t.Errorf("now: %s", b[:min(len(b), 300)])
	}
}
