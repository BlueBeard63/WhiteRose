package main

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"whiterose/daemon/internal/config"
	"whiterose/daemon/internal/edid/edidtest"
)

// fakeSysfs writes a DRM tree to a temp dir: a laptop panel plus one monitor.
func fakeSysfs(t *testing.T) string {
	t.Helper()
	root := t.TempDir()
	write := func(rel string, data []byte) {
		p := filepath.Join(root, rel)
		if err := os.MkdirAll(filepath.Dir(p), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(p, data, 0o644); err != nil {
			t.Fatal(err)
		}
	}
	write("card1-eDP-1/status", []byte("connected\n"))
	write("card1-eDP-1/edid", edidtest.Builder{Manufacturer: "BOE", ProductCode: 0x0BCA, Name: "NE160QDM"}.Bytes())
	write("card1-DP-1/status", []byte("connected\n"))
	write("card1-DP-1/edid", edidtest.Builder{Manufacturer: "DEL", ProductCode: 0x42AC, SerialString: "7F3K2Q3", Name: "DELL U2723QE"}.Bytes())
	write("card1-DP-2/status", []byte("disconnected\n"))
	return root
}

func TestMonitorsTable(t *testing.T) {
	var out, errb bytes.Buffer
	if code := run([]string{"monitors", "-sysfs", fakeSysfs(t)}, &out, &errb); code != 0 {
		t.Fatalf("exit %d: %s", code, errb.String())
	}
	got := out.String()
	for _, want := range []string{"CONNECTOR", "eDP-1", "builtin", "DP-1", "edid:", "strong", "DELL U2723QE"} {
		if !strings.Contains(got, want) {
			t.Errorf("output missing %q:\n%s", want, got)
		}
	}
	if strings.Contains(got, "DP-2") {
		t.Errorf("disconnected connector listed:\n%s", got)
	}
}

func TestMonitorsJSONIDsAreValidConfigIDs(t *testing.T) {
	var out, errb bytes.Buffer
	if code := run([]string{"monitors", "-json", "-sysfs", fakeSysfs(t)}, &out, &errb); code != 0 {
		t.Fatalf("exit %d: %s", code, errb.String())
	}
	var rows []monitorJSON
	if err := json.Unmarshal(out.Bytes(), &rows); err != nil {
		t.Fatalf("bad JSON: %v\n%s", err, out.String())
	}
	if len(rows) != 2 {
		t.Fatalf("got %d rows", len(rows))
	}
	for _, r := range rows {
		// Whatever the scanner prints must be pasteable into displays.toml.
		if !config.ValidID(r.ID) {
			t.Errorf("%s: id %q would be rejected by the config loader", r.Connector, r.ID)
		}
	}
}

func TestCheckConfig(t *testing.T) {
	dir := t.TempDir()
	good := filepath.Join(dir, "good.toml")
	bad := filepath.Join(dir, "bad.toml")
	os.WriteFile(good, []byte("[[monitor]]\nid = \"builtin\"\nscale = 1.5\n"), 0o644)
	os.WriteFile(bad, []byte("[[monitor]]\nid = \"builtin\"\nscale = 9\n"), 0o644)

	var out, errb bytes.Buffer
	if code := run([]string{"check-config", good}, &out, &errb); code != 0 {
		t.Errorf("good config: exit %d: %s", code, errb.String())
	}
	out.Reset()
	errb.Reset()
	if code := run([]string{"check-config", bad}, &out, &errb); code != 1 {
		t.Errorf("bad config: exit %d, want 1", code)
	}
	if !strings.Contains(errb.String(), "scale") {
		t.Errorf("error should explain the problem: %q", errb.String())
	}
}

func TestUnknownCommand(t *testing.T) {
	var out, errb bytes.Buffer
	if code := run([]string{"frobnicate"}, &out, &errb); code != 2 {
		t.Errorf("exit %d, want 2", code)
	}
	if code := run(nil, &out, &errb); code != 2 {
		t.Errorf("no args: exit %d, want 2", code)
	}
}
