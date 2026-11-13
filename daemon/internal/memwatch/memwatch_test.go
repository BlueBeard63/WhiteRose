package memwatch

import (
	"testing"
	"time"
)

func TestParseUnit(t *testing.T) {
	for unit, want := range map[string]string{
		`app-whiterose-org.gnome.Calculator-3678141-0.scope`:             "org.gnome.Calculator",
		`app-whiterose-whiterose\x2dsettings-3678141-2.scope`:            "whiterose-settings",
		`app-gnome-org.gnome.Evolution\x2dalarm\x2dnotify-3588340.scope`: "org.gnome.Evolution-alarm-notify",
		`app-flatpak-com.bitwarden.desktop-12.scope`:                     "com.bitwarden.desktop",
		`app-gnome-firefox-123.scope`:                                    "firefox",
	} {
		if got, ok := ParseUnit(unit); !ok || got != want {
			t.Errorf("%s: %q %v, want %q", unit, got, ok, want)
		}
	}
	for _, not := range []string{`app-gnome\x2dsession\x2dmanager.slice`, "session-3.scope", "app-.scope", "app-gnome.scope"} {
		if id, ok := ParseUnit(not); ok {
			t.Errorf("%s: %q", not, id)
		}
	}
}

func TestProtected(t *testing.T) {
	if !Protected("com.mitchellh.ghostty", nil, "ghostty -e x") || !Protected("org.mozilla.Firefox", []string{"firefox"}, "") {
		t.Error("not protected")
	}
	if Protected("org.gnome.Calculator", []string{"firefox"}, "foot") {
		t.Error("protected")
	}
}

func TestJudge(t *testing.T) {
	const gb = 1 << 30
	for _, c := range []struct {
		p     PSI
		avail int64
		want  Level
	}{
		{PSI{}, 8 * gb, Calm},
		{PSI{Some10: 12}, 8 * gb, Tight},
		{PSI{}, gb + gb/2, Tight}, // 9% of 16 GB
		{PSI{Some10: 40, Full10: 25}, 4 * gb, Critical},
		{PSI{}, gb / 4, Critical},
	} {
		if got := Judge(c.p, c.avail, 16*gb); got != c.want {
			t.Errorf("%+v %d: %v, want %v", c.p, c.avail, got, c.want)
		}
	}
	p := ParsePSI("some avg10=12.50 avg60=3.00 avg300=1.00 total=1\nfull avg10=0.75 avg60=0 avg300=0 total=2\n")
	if p.Some10 != 12.5 || p.Full10 != 0.75 {
		t.Errorf("%+v", p)
	}
}

func TestLeaks(t *testing.T) {
	const mb = 1 << 20
	var tr Tracker
	t0 := time.Date(2026, 11, 13, 12, 0, 0, 0, time.UTC)
	none := func(string) bool { return false }
	var apps []App
	// 11 minutes, one reading every 30 s: firefox grows 1 → ~2.3 GB (with a
	// dip), the calculator stays flat, the editor grows but is focused.
	for i := 0; i <= 22; i++ {
		ff := int64(1024+60*i) * mb
		if i == 10 {
			ff -= 30 * mb
		}
		apps = []App{
			{Unit: "app-gnome-firefox-1.scope", ID: "firefox", Bytes: ff},
			{Unit: "app-whiterose-org.gnome.Calculator-1-0.scope", ID: "org.gnome.Calculator", Bytes: 80 * mb},
			{Unit: "app-whiterose-code-1-1.scope", ID: "code", Bytes: int64(500+40*i) * mb},
		}
		tr.Add(t0.Add(time.Duration(i)*30*time.Second), apps)
	}
	s := tr.Suspects(apps, 16*1024*mb, "code", none)
	if len(s) != 1 || s[0].ID != "firefox" || !s[0].Growing {
		t.Fatalf("%+v", s)
	}
	// Not focused, the editor is a suspect too; firefox (bigger) first.
	if s := tr.Suspects(apps, 16*1024*mb, "", none); len(s) != 2 || s[0].ID != "firefox" {
		t.Errorf("%+v", s)
	}
	// Protected: never.
	if s := tr.Suspects(apps, 16*1024*mb, "code", func(id string) bool { return id == "firefox" }); len(s) != 0 {
		t.Errorf("%+v", s)
	}
	// Half of memory is a suspect without growing.
	if s := tr.Suspects(apps, 4*1024*mb, "code", none); len(s) != 1 || s[0].ID != "firefox" {
		t.Errorf("half: %+v", s)
	}
	// Gone: forgotten.
	tr.Add(t0.Add(12*time.Minute), apps[1:])
	if tr.Growing("app-gnome-firefox-1.scope") {
		t.Error("a closed app is still growing")
	}
	// Worst with no suspect: the biggest unprotected app.
	w, ok := tr.Worst(apps[1:2], 16*1024*mb, "", none)
	if !ok || w.ID != "org.gnome.Calculator" || w.Reason != "biggest" {
		t.Errorf("%+v", w)
	}
	if _, ok := tr.Worst(apps[1:2], 16*1024*mb, "", func(string) bool { return true }); ok {
		t.Error("named a protected app")
	}
}

func TestGrowingNeedsTime(t *testing.T) {
	var tr Tracker
	t0 := time.Now()
	for i := range 10 {
		tr.Add(t0.Add(time.Duration(i)*30*time.Second), []App{{Unit: "u", ID: "x", Bytes: int64(1+i) << 30}})
	}
	if tr.Growing("u") {
		t.Error("growing after 4.5 minutes")
	}
}
