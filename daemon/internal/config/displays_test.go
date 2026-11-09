package config_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"whiterose/daemon/internal/config"
)

func TestLoadDisplaysExample(t *testing.T) {
	d, err := config.LoadDisplays(filepath.Join("testdata", "displays.toml"))
	if err != nil {
		t.Fatal(err)
	}
	if len(d.Monitors) != 3 || len(d.Profiles) != 3 {
		t.Fatalf("got %d monitors, %d profiles", len(d.Monitors), len(d.Profiles))
	}
	if got := d.Monitor("edid:3f9a1c2b7e44"); got.Label != "Desk left" || got.Scale != 1.25 {
		t.Errorf("Monitor lookup: %+v", got)
	}
	if got := d.Monitor("edid:000000000000"); got.ID != "edid:000000000000" || got.Scale != 0 {
		t.Errorf("unknown monitor should get defaults, got %+v", got)
	}
	desk := d.Profiles[0]
	if desk.Outputs[0].Position != [2]int{0, 152} {
		t.Errorf("position = %v", desk.Outputs[0].Position)
	}
}

func TestLoadDisplaysMissingFile(t *testing.T) {
	_, err := config.LoadDisplays(filepath.Join(t.TempDir(), "nope.toml"))
	if !os.IsNotExist(err) {
		t.Errorf("err = %v, want not-exist", err)
	}
}

func TestMatchProfile(t *testing.T) {
	d, err := config.LoadDisplays(filepath.Join("testdata", "displays.toml"))
	if err != nil {
		t.Fatal(err)
	}
	tests := []struct {
		name      string
		connected []string
		want      string
		ok        bool
	}{
		{"exact set", []string{"builtin", "edid:3f9a1c2b7e44", "edid:91c04d7a22f0"}, "desk", true},
		{"order does not matter", []string{"edid:91c04d7a22f0", "builtin", "edid:3f9a1c2b7e44"}, "desk", true},
		{"subset picks its own profile", []string{"edid:3f9a1c2b7e44", "builtin"}, "desk, lid closed", true},
		{"laptop only", []string{"builtin"}, "laptop", true},
		{"unknown monitor added: no match", []string{"builtin", "edid:aaaaaaaaaaaa"}, "", false},
		{"nothing connected", nil, "", false},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			p, ok := d.MatchProfile(tt.connected)
			if ok != tt.ok || p.Name != tt.want {
				t.Errorf("got (%q, %v), want (%q, %v)", p.Name, ok, tt.want, tt.ok)
			}
		})
	}
}

func TestMatchProfileFirstWins(t *testing.T) {
	d := parse(t, `
[[profile]]
name = "first"
  [[profile.output]]
  id = "builtin"
[[profile]]
name = "second"
  [[profile.output]]
  id = "builtin"
`)
	if p, _ := d.MatchProfile([]string{"builtin"}); p.Name != "first" {
		t.Errorf("got %q, want first", p.Name)
	}
}

func TestParseDisplaysErrors(t *testing.T) {
	tests := []struct {
		name string
		toml string
		want []string // substrings that must all appear in the error
	}{
		{"unknown key catches typos", `
[[monitor]]
id = "builtin"
sacle = 2`, []string{"unknown keys", "monitor.sacle"}},
		{"bad toml", `[[monitor]`, []string{"toml"}},
		{"missing id", `
[[monitor]]
label = "x"`, []string{"monitor[0].id: required"}},
		{"bad id shape", `
[[monitor]]
id = "DP-1"`, []string{`invalid monitor id "DP-1"`}},
		{"duplicate id", `
[[monitor]]
id = "builtin"
[[monitor]]
id = "builtin"`, []string{"monitor[1].id", "already used by monitor[0]"}},
		{"bad mode", `
[[monitor]]
id = "builtin"
mode = "1080p"`, []string{"monitor[0].mode", "invalid mode"}},
		{"zero refresh", `
[[monitor]]
id = "builtin"
mode = "1920x1080@0"`, []string{"refresh rate is 0"}},
		{"scale out of range", `
[[monitor]]
id = "builtin"
scale = 7`, []string{"between 0.5 and 4"}},
		{"scale off the 1/120 grid", `
[[monitor]]
id = "builtin"
scale = 1.333`, []string{"multiple of 1/120"}},
		{"bad transform", `
[[monitor]]
id = "builtin"
transform = "45"`, []string{"monitor[0].transform"}},
		{"bad vrr", `
[[monitor]]
id = "builtin"
vrr = "maybe"`, []string{"monitor[0].vrr"}},
		{"profile without outputs", `
[[profile]]
name = "empty"`, []string{"profile[0]: needs at least one output"}},
		{"profile all disabled", `
[[profile]]
name = "dark"
  [[profile.output]]
  id = "builtin"
  disabled = true`, []string{"every output is disabled"}},
		{"duplicate output in profile", `
[[profile]]
name = "p"
  [[profile.output]]
  id = "builtin"
  [[profile.output]]
  id = "builtin"`, []string{`"builtin" listed twice`}},
		{"duplicate profile names", `
[[profile]]
name = "p"
  [[profile.output]]
  id = "builtin"
[[profile]]
name = "p"
  [[profile.output]]
  id = "builtin"`, []string{"profile[1].name", "already used by profile[0]"}},
		{"reports every problem at once", `
[[monitor]]
id = "builtin"
scale = 9
vrr = "x"
[[profile]]
name = ""
  [[profile.output]]
  id = "nope"`, []string{"monitor[0].scale", "monitor[0].vrr", "profile[0].name", "profile[0].output[0].id"}},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			_, err := config.ParseDisplays(strings.NewReader(tt.toml))
			if err == nil {
				t.Fatal("expected an error")
			}
			for _, w := range tt.want {
				if !strings.Contains(err.Error(), w) {
					t.Errorf("error %q does not mention %q", err, w)
				}
			}
		})
	}
}

func TestValidScales(t *testing.T) {
	for _, s := range []string{"0.5", "1", "1.25", "1.5", "1.6", "1.75", "2", "4"} {
		_ = parse(t, "[[monitor]]\nid = \"builtin\"\nscale = "+s+"\n")
	}
}

func TestParseMode(t *testing.T) {
	tests := []struct {
		in   string
		want config.Mode
		ok   bool
		err  bool
	}{
		{"", config.Mode{}, false, false},
		{"preferred", config.Mode{}, false, false},
		{"2560x1440", config.Mode{2560, 1440, 0}, true, false},
		{"2560x1440@165", config.Mode{2560, 1440, 165}, true, false},
		{"1920x1080@59.94", config.Mode{1920, 1080, 59.94}, true, false},
		{"0x1080", config.Mode{}, false, true},
		{"1920x1080@", config.Mode{}, false, true},
		{"1920*1080", config.Mode{}, false, true},
	}
	for _, tt := range tests {
		got, ok, err := config.ParseMode(tt.in)
		if (err != nil) != tt.err || ok != tt.ok || got != tt.want {
			t.Errorf("ParseMode(%q) = %+v, %v, %v", tt.in, got, ok, err)
		}
	}
}

func TestValidID(t *testing.T) {
	for _, ok := range []string{"builtin", "edid:3f9a1c2b7e44", "edid:3f9a1c2b7e44@DP-1", "edid:3f9a1c2b7e44@mst:62-1-8", "connector:HDMI-A-1"} {
		if !config.ValidID(ok) {
			t.Errorf("%q should be valid", ok)
		}
	}
	for _, bad := range []string{"", "DP-1", "edid:XYZ", "edid:3f9a1c2b7e4", "edid:3F9A1C2B7E44", "builtin@DP-1", "connector:"} {
		if config.ValidID(bad) {
			t.Errorf("%q should be invalid", bad)
		}
	}
}

func parse(t *testing.T, s string) *config.Displays {
	t.Helper()
	d, err := config.ParseDisplays(strings.NewReader(s))
	if err != nil {
		t.Fatal(err)
	}
	return d
}
