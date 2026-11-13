package report

import (
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"whiterose/daemon/internal/crash"
	"whiterose/daemon/internal/snapshot"
)

func input() Input {
	return Input{
		Draft: Draft{Snapshot: "1794571766000-shell", Kind: "crashed", Where: "The bar", Title: "The shell crashed (signal 11)",
			What: "I opened quick settings and the bar went away."},
		Record: crash.Record{What: crash.Shell, Signal: 11, Time: 1794571766000},
		Snap: &snapshot.Snapshot{
			WhiteRose: snapshot.WhiteRose{Version: "0.3.1", Uptime: 3600, Xwayland: true},
			System: snapshot.System{OS: "Ubuntu 26.04 LTS", Kernel: "6.17", CPU: "AMD Ryzen 7", Threads: 16, MemoryMB: 15000,
				GPUs:     []snapshot.GPU{{Vendor: "AMD", Device: "0x15bf", Driver: "amdgpu"}},
				Monitors: []snapshot.Monitor{{Name: "BOE 0x0BCA", Mode: "2880x1800 @ 120 Hz", Scale: 1.5}}},
			Load:    &snapshot.Load{Avg: [3]float64{1, 2, 3}, MemUsedMB: 9000, MemTotalMB: 15000, TopCPU: []snapshot.Process{{Name: "firefox", CPU: 80, RSSMB: 2000}}},
			Windows: &snapshot.Windows{Count: 2, Apps: map[string]int{"firefox": 2}, Focused: "firefox"},
			Log:     []string{"a", "b", "panicked at bar.rs"},
		},
		Output: "Segmentation fault\n", Build: "verified", BuildID: strings.Repeat("ab", 32), Repo: "me/whiterose",
		Now: time.Date(2026, 11, 13, 14, 0, 0, 0, time.Local), MaxLines: 2,
	}
}

func TestFile(t *testing.T) {
	in := input()
	f := File(in)
	for _, want := range []string{"Title: The shell crashed (signal 11)", "Kind: Crashed", "Where: The bar",
		"The shell crashed (signal 11, SIGSEGV)", "I opened quick settings", "Ubuntu 26.04 LTS", "GPU: AMD 0x15bf (amdgpu)",
		"2880x1800 @ 120 Hz", "firefox", "Segmentation fault", "b\npanicked at bar.rs"} {
		if !strings.Contains(f, want) {
			t.Errorf("no %q in\n%s", want, f)
		}
	}
	if strings.Contains(f, "\na\n") {
		t.Error("more log lines than MaxLines")
	}
	// Switched off: left out.
	in.Draft.Off = []string{"load", "windows", "log", "output"}
	f = File(in)
	for _, gone := range []string{"Load at the time", "firefox", "panicked", "Segmentation"} {
		if strings.Contains(f, gone) {
			t.Errorf("%q is in, switched off", gone)
		}
	}
}

func TestReadable(t *testing.T) {
	in := input()
	in.Snap.WhiteRose.Uptime = 3661
	in.Snap.Windows = &snapshot.Windows{Count: 0, Apps: map[string]int{}}
	in.Snap.Load.TopCPU = []snapshot.Process{{Name: "a-very-long-process-name-indeed", CPU: 2}}
	f := File(in)
	for _, want := range []string{"session up 1 h 1 min", "0 open\n", "a-very-long-process-nam… "} {
		if !strings.Contains(f, want) {
			t.Errorf("no %q in\n%s", want, f)
		}
	}
	if strings.Contains(f, "focused:") {
		t.Error("an empty focused")
	}
	if uptime(59*60) != "59 min" || uptime(2*86400+3*3600) != "2 d 3 h" {
		t.Error(uptime(59*60), uptime(2*86400+3*3600))
	}
}

func TestFormURL(t *testing.T) {
	in := input()
	in.Draft.What = strings.Repeat("x", 5000)
	u, err := url.Parse(FormURL(in, "whiterose-report-x.txt"))
	if err != nil || u.Host != "github.com" || u.Path != "/me/whiterose/issues/new" {
		t.Fatalf("%v %v", u, err)
	}
	q := u.Query()
	if q.Get("template") != "bug.yml" || q.Get("version") != "0.3.1" || q.Get("build") != in.BuildID || q.Get("title") != in.Draft.Title {
		t.Errorf("%v", q)
	}
	if !strings.Contains(q.Get("what"), "the rest is in whiterose-report-x.txt") || len(q.Get("what")) > 2600 {
		t.Errorf("what not cut: %d", len(q.Get("what")))
	}
	if !strings.Contains(q.Get("system"), "Ubuntu") || strings.Contains(q.Get("system"), "Most CPU") {
		t.Errorf("system %q", q.Get("system"))
	}
	if len(u.String()) > 8000 {
		t.Errorf("address %d long", len(u.String()))
	}
	in.Draft.Off = []string{"system"}
	if q := mustQuery(t, FormURL(in, "f")); strings.Contains(q.Get("system"), "Ubuntu") {
		t.Error("system switched off, yet in the form")
	}
}

func mustQuery(t *testing.T, s string) url.Values {
	u, err := url.Parse(s)
	if err != nil {
		t.Fatal(err)
	}
	return u.Query()
}

func TestDownloadsDir(t *testing.T) {
	home := t.TempDir()
	if d := DownloadsDir(home); d != filepath.Join(home, "Downloads") {
		t.Errorf("%s", d)
	}
	os.MkdirAll(filepath.Join(home, ".config"), 0o755)
	os.WriteFile(filepath.Join(home, ".config", "user-dirs.dirs"), []byte("XDG_DOWNLOAD_DIR=\"$HOME/Téléchargements\"\n"), 0o644)
	if d := DownloadsDir(home); d != filepath.Join(home, "Téléchargements") {
		t.Errorf("%s", d)
	}
}
