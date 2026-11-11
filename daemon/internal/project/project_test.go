package project

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestNormalize(t *testing.T) {
	for in, want := range map[string]string{
		"0.2.0": "0.2.0", "v0.2.0": "0.2.0", "0.6": "0.6.0", "1": "1.0.0", "v0.6-alpha-12": "0.6.0-alpha.12",
		"0.6.0-alpha.12": "0.6.0-alpha.12", "0.6.0-RC.1": "0.6.0-rc.1", "0.6.0~beta.2": "0.6.0-beta.2", " 0.07.1 ": "0.7.1",
	} {
		if got, err := Normalize(in); err != nil || got != want {
			t.Errorf("Normalize(%q) = %q, %v; want %q", in, got, err, want)
		}
	}
	for _, bad := range []string{"", "latest", "0.6.0-alpha 1", "0.6.0-al/pha", "1.2.3.4", "0.6.0-", "../0.6"} {
		if got, err := Normalize(bad); err == nil {
			t.Errorf("Normalize(%q) = %q, want an error", bad, got)
		}
	}
	if !Prerelease("0.6.0-alpha.12") || Prerelease("0.6.0") {
		t.Error("Prerelease")
	}
}

func write(t *testing.T, text string) string {
	t.Helper()
	p := filepath.Join(t.TempDir(), "project.yaml")
	if err := os.WriteFile(p, []byte(text), 0o644); err != nil {
		t.Fatal(err)
	}
	return p
}

func TestLoadNotesAndChangelog(t *testing.T) {
	p, err := Load(write(t, `
name: WhiteRose
repo: BlueBeard63/WhiteRose
version: v0.6-alpha-12

releases:
  - version: 0.6.0-alpha.12
    date: 2026-12-01
    title: Login
    summary: Trying out the display manager.
    added:
      - A display manager
      - Session picker
    fixed: [Wi-Fi password under its row]
    contributors: [BlueBeard63, "@someone-else"]
    notes: |
      Known issue: no guest account yet.

  - version: 0.2   # unquoted: YAML would make it a number; it's read as written
    date: 2026-11-11
    title: Files
    changed: [No dialogs]
`))
	if err != nil {
		t.Fatal(err)
	}
	if p.Version != "0.6.0-alpha.12" || p.Repo != "BlueBeard63/WhiteRose" {
		t.Fatalf("%+v", p)
	}
	notes, err := p.Notes(p.Version)
	if err != nil {
		t.Fatal(err)
	}
	// What's new first (Settings › Updates shows the first bullets).
	if !strings.HasPrefix(notes, "Trying out the display manager.\n\n## Added\n\n- A display manager\n") {
		t.Errorf("notes start:\n%s", notes)
	}
	for _, want := range []string{"## Fixed\n\n- Wi-Fi password", "Known issue", "Thanks to @BlueBeard63, @someone-else.",
		"whiterose-core-0.6.0-alpha.12_0.6.0-alpha.12_amd64.deb", "This is a pre-release",
		"curl -fsSL https://github.com/BlueBeard63/WhiteRose/releases/download/v0.6.0-alpha.12/install.sh | sh"} {
		if !strings.Contains(notes, want) {
			t.Errorf("notes lack %q:\n%s", want, notes)
		}
	}
	if strings.Contains(notes, "## Changed") {
		t.Error("empty sections are left out")
	}
	if _, err := p.Notes("0.3.0"); err != ErrNoEntry {
		t.Errorf("a version without an entry: %v", err)
	}
	log := p.Changelog()
	if p.Release("0.2.0") == nil {
		t.Error("an unquoted 0.2 is 0.2.0")
	}
	if !strings.Contains(log, "## 0.6.0-alpha.12 · Login (2026-12-01)") || strings.Index(log, "0.6.0-alpha.12") > strings.Index(log, "## 0.2.0") {
		t.Errorf("changelog:\n%s", log)
	}
	if !strings.Contains(log, "Contributors: @BlueBeard63, @someone-else") {
		t.Errorf("changelog contributors:\n%s", log)
	}
}

func TestLoadRefuses(t *testing.T) {
	for name, text := range map[string]string{
		"a bad version":       "version: next",
		"a typo'd key":        "version: 0.2.0\nverison: x",
		"a bad date":          "version: 0.2.0\nreleases:\n  - version: 0.2.0\n    date: 11/11/2026",
		"the same twice":      "version: 0.2.0\nreleases:\n  - version: 0.2.0\n  - version: v0.2",
		"a bad contributor":   "version: 0.2.0\nreleases:\n  - version: 0.2.0\n    contributors: [Jack Morrison]",
		"a bad repo":          "repo: not a repo\nversion: 0.2.0",
		"a typo in a release": "version: 0.2.0\nreleases:\n  - version: 0.2.0\n    aded: [x]",
	} {
		if _, err := Load(write(t, text)); err == nil {
			t.Errorf("%s: accepted", name)
		}
	}
}
