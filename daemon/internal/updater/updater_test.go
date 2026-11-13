package updater

import (
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"whiterose/daemon/internal/launch"
	"whiterose/daemon/internal/update"
)

// A fake GitHub with release `ver`, signed by `priv`.
func fakeGitHub(t *testing.T, ver, priv string) update.Source {
	t.Helper()
	dir := t.TempDir()
	names := []string{"whiterose-core-" + ver + "_" + ver + "_amd64.deb", "whiterose_" + ver + "_amd64.deb"}
	files := map[string][]byte{}
	for i, n := range names {
		b := []byte(strings.Repeat("package ", 100*(i+1)))
		os.WriteFile(filepath.Join(dir, n), b, 0o644)
		files[n] = b
	}
	sums, _ := update.MakeSums(dir, names)
	sig, _ := update.Sign(priv, sums)
	files["SHA256SUMS"], files["SHA256SUMS.sig"] = sums, []byte(sig)
	var srv *httptest.Server
	srv = httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/repos/me/whiterose/releases/latest" {
			var assets []update.Asset
			for n, b := range files {
				assets = append(assets, update.Asset{Name: n, Size: int64(len(b)), URL: srv.URL + "/dl/" + n})
			}
			json.NewEncoder(w).Encode(update.Release{Tag: "v" + ver, Notes: "- New window switcher\n- Faster", Published: time.Date(2026, 11, 9, 0, 0, 0, 0, time.UTC), Assets: assets})
			return
		}
		if b, ok := files[strings.TrimPrefix(r.URL.Path, "/dl/")]; ok {
			w.Write(b)
			return
		}
		http.NotFound(w, r)
	}))
	t.Cleanup(srv.Close)
	return update.Source{Repo: "me/whiterose", API: srv.URL}
}

func installVersion(t *testing.T, root, v string) {
	t.Helper()
	os.MkdirAll(filepath.Join(root, v, "bin"), 0o755)
	os.WriteFile(filepath.Join(root, v, "bin", "whiterose-wm"), []byte("#!/bin/sh\n"), 0o755)
}

func setup(t *testing.T, running string) (*Manager, *[]any) {
	t.Helper()
	priv, pub, _ := update.NewKey()
	var events []any
	m := &Manager{
		Src: fakeGitHub(t, "0.5.0", priv), Public: pub, Kind: "deb",
		Root: t.TempDir(), State: t.TempDir(), Cache: t.TempDir(), Running: running, Launcher: true,
		Emit: func(e any) { events = append(events, e) },
	}
	if running != "" {
		installVersion(t, m.Root, running)
	}
	m.Install = func(ctx context.Context, dir, keep string, line func(string)) error {
		// What the root helper does: check the download again, install it.
		if _, err := update.VerifyDir(dir, "deb", pub); err != nil {
			return err
		}
		installVersion(t, m.Root, "0.5.0")
		return nil
	}
	return m, &events
}

func TestNotSetUp(t *testing.T) {
	m, _ := setup(t, "")
	if v := m.Status(); v.Supported || !strings.Contains(v.Why, "build being worked on") {
		t.Errorf("%+v", v)
	}
	if m.Check(context.Background(), true) == nil {
		t.Error("checked without being set up")
	}
	m.Running, m.Public = "0.4.2", ""
	if v := m.Status(); v.Supported || !strings.Contains(v.Why, "release key") {
		t.Errorf("%+v", v)
	}
}

func TestCheckDownloadInstallApply(t *testing.T) {
	m, events := setup(t, "0.4.2")
	ctx := context.Background()
	if !m.Due(24 * time.Hour) {
		t.Error("never checked, yet not due")
	}

	// Metered: found, not downloaded.
	m.Metered = func() bool { return true }
	if err := m.Check(ctx, true); err != nil {
		t.Fatal(err)
	}
	v := m.Status()
	if v.Release == nil || v.Release.Version != "0.5.0" || v.Release.State != "available" || len(v.Release.Changes) != 2 {
		t.Fatalf("%+v", v.Release)
	}
	if m.Due(24 * time.Hour) {
		t.Error("just checked, yet due")
	}

	// Not metered: downloaded and verified straight away.
	m.Metered = func() bool { return false }
	if err := m.Check(ctx, true); err != nil {
		t.Fatal(err)
	}
	v = m.Status()
	r := v.Release
	if r.State != "downloaded" || !r.Steps[0].Done || !r.Steps[1].Done || !r.Steps[2].Done || r.Steps[3].Done || !r.Steps[3].Active {
		t.Fatalf("%+v", r)
	}
	if !strings.Contains(r.Steps[1].Detail, "Release key") || r.Steps[3].Detail != "0.4.2 kept on disk" {
		t.Errorf("%+v", r.Steps)
	}
	if len(*events) == 0 {
		t.Error("the page wasn't told")
	}

	// Installed beside the running one: ready, applies at the next start.
	if err := m.InstallNow(ctx); err != nil {
		t.Fatal(err)
	}
	r = m.Status().Release
	if r.State != "ready" || !r.Pending || !r.Steps[3].Done || !r.Steps[4].Active {
		t.Errorf("%+v", r)
	}
	if _, err := os.Stat(m.Dir("0.5.0")); err == nil {
		t.Error("the download was kept after installing")
	}
	st := launch.Load(launch.StateFile(m.State))
	if st.Pending != "0.5.0" {
		t.Errorf("launcher state %+v", st)
	}
	// Restart session now.
	restarted := false
	m.Restart = func() error { restarted = true; return nil }
	if err := m.RestartSession(); err != nil || !restarted {
		t.Error(err)
	}
	m.Launcher = false
	if m.RestartSession() == nil {
		t.Error("restarted without a launcher to start it again")
	}
}

func TestInstallRefusesTamperedDownload(t *testing.T) {
	m, _ := setup(t, "0.4.2")
	ctx := context.Background()
	if err := m.Check(ctx, true); err != nil {
		t.Fatal(err)
	}
	// Changed after the download (by something running as you).
	entries, _ := os.ReadDir(m.Dir("0.5.0"))
	for _, e := range entries {
		if strings.HasSuffix(e.Name(), ".deb") {
			os.WriteFile(filepath.Join(m.Dir("0.5.0"), e.Name()), []byte("evil"), 0o600)
		}
	}
	if err := m.InstallNow(ctx); err == nil {
		t.Fatal("a changed download installed")
	}
	if v := m.Status(); v.Release.State != "downloaded" || v.Error == "" {
		t.Errorf("%+v %q", v.Release, v.Error)
	}
}

// A newer version installed outside the page (install.sh, apt): shown as
// waiting for the next start, even when the release check knows nothing newer.
func TestWaiting(t *testing.T) {
	m, _ := setup(t, "0.4.1")
	f := launch.StateFile(m.State)
	launch.Save(f, launch.State{Current: "0.4.1"})
	if v := m.Status(); v.Waiting != "" {
		t.Errorf("nothing newer: %+v", v)
	}
	installVersion(t, m.Root, "0.4.2")
	installVersion(t, m.Root, "0.4.3")
	// A stale pending one: the newest is what starts.
	launch.Save(f, launch.State{Current: "0.4.1", Pending: "0.4.2"})
	if v := m.Status(); v.Waiting != "0.4.3" {
		t.Errorf("waiting: %+v", v)
	}
	// Not when you chose to stay off it.
	launch.Save(f, launch.State{Current: "0.4.1", Skip: "0.4.3"})
	if v := m.Status(); v.Waiting != "" {
		t.Errorf("skipped: %+v", v)
	}
	// Looking doesn't change the state.
	if st := launch.Load(f); st.Pending != "" || st.Attempts != 0 {
		t.Errorf("state changed: %+v", st)
	}
}

func TestRollbackAndPrompt(t *testing.T) {
	m, _ := setup(t, "0.5.0")
	installVersion(t, m.Root, "0.4.2")
	installVersion(t, m.Root, "0.3.0") // (left over; not offered)
	v := m.Status()
	if v.Previous != "0.4.2" || v.RolledTo != "" || !v.Rollback {
		t.Fatalf("%+v", v)
	}
	if err := m.Rollback(); err != nil {
		t.Fatal(err)
	}
	if v := m.Status(); v.RolledTo != "0.4.2" {
		t.Errorf("%+v", v)
	}
	st := launch.Load(launch.StateFile(m.State))
	if st.Current != "0.4.2" || st.Skip != "0.5.0" {
		t.Errorf("%+v", st)
	}
	m.UndoRollback()
	if v := m.Status(); v.RolledTo != "" {
		t.Errorf("undo: %+v", v)
	}

	if err := m.SetAutoRollback(false); err != nil || m.Status().Rollback {
		t.Error("automatic rollback still on")
	}

	// A failed start: the prompt's view, then Stay.
	f := launch.StateFile(m.State)
	launch.Save(f, launch.FailedTry(launch.State{Current: "0.4.2", Pending: "0.5.0", Attempts: 2}, "0.5.0", "compositor exited during start (signal 11)"))
	m.Running = "0.4.2"
	v = m.Status()
	if v.Failure == nil || v.Failure.Attempts != 2 || v.Failure.BackOn != "0.4.2" || !strings.HasSuffix(v.Failure.Log, "update-0.5.0.log") {
		t.Fatalf("%+v", v.Failure)
	}
	if err := m.Stay(); err != nil || m.Status().Failure != nil {
		t.Error("the prompt didn't go away")
	}
	// Try again: two more tries, starting now.
	launch.Save(f, launch.FailedTry(launch.Load(f), "0.5.0", "again"))
	m.Restart = func() error { return errors.New("no compositor") }
	if err := m.TryAgain(); err == nil {
		t.Error("restart failure hidden")
	}
	if st := launch.Load(f); st.Pending != "0.5.0" || st.Attempts != 0 || st.Failed != "" {
		t.Errorf("%+v", st)
	}
}
