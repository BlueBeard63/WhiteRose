package migrate

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"whiterose/daemon/internal/store"
)

func TestOldFilesAreImportedOnceAndKept(t *testing.T) {
	root := t.TempDir()
	d := Dirs{Config: filepath.Join(root, "config"), State: filepath.Join(root, "state")}
	write := func(p, s string) {
		_ = os.MkdirAll(filepath.Dir(p), 0o755)
		if err := os.WriteFile(p, []byte(s), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	write(filepath.Join(d.Config, "dock.json"), `{"pinned":["firefox_firefox","code_code"]}`)
	write(filepath.Join(d.Config, "launcher.json"), `{"launches":{"firefox_firefox":3},"commands":["htop"],"calculations":[{"expr":"2+2","result":"4"}]}`)
	write(filepath.Join(d.State, "quick.json"), `{"doNotDisturb":true,"nightLightTemperature":3500,"nightLightUntil":0}`)
	write(filepath.Join(d.Config, "weather.toml"), "city = \"London\"\nunits = \"fahrenheit\"\n")
	write(filepath.Join(d.Config, "calendars.toml"), "[[calendar]]\nname = \"Personal\"\nurl = \"https://cal.example/private-abc/basic.ics\"\n[[calendar]]\nname = \"Work\"\npath = \"~/work.ics\"\ncolor = \"#123456\"\n")
	example, err := os.ReadFile(filepath.Join("..", "config", "testdata", "displays.toml"))
	if err != nil {
		t.Fatal(err)
	}
	write(filepath.Join(d.Config, "displays.toml"), string(example))

	st, err := store.Open(filepath.Join(root, "data", "settings.db"))
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()
	secrets := &store.Resolver{Fallback: st.Fallback()}
	r := Run(st, secrets, d)
	if len(r.Failed) != 0 || len(r.Imported) != 6 {
		t.Fatalf("imported %v, failed %v", r.Imported, r.Failed)
	}
	var pinned []string
	var launches map[string]int
	var dnd bool
	var temp int
	var city, units string
	_ = st.GetInto("dock.pinned", &pinned)
	_ = st.GetInto("launcher.launches", &launches)
	_ = st.GetInto("quick.doNotDisturb", &dnd)
	_ = st.GetInto("quick.nightLight.temperature", &temp)
	_ = st.GetInto("weather.city", &city)
	_ = st.GetInto("weather.units", &units)
	var disp struct {
		Profile []struct {
			Name string `json:"name"`
		} `json:"profile"`
	}
	if err := st.GetInto("displays", &disp); err != nil || len(disp.Profile) == 0 {
		t.Fatalf("display profiles: %+v %v", disp, err)
	}
	if len(pinned) != 2 || launches["firefox_firefox"] != 3 || !dnd || temp != 3500 || city != "London" || units != "fahrenheit" {
		t.Fatalf("values: %v %v %v %v %q %q", pinned, launches, dnd, temp, city, units)
	}
	var cals []store.CalendarSource
	_ = st.GetInto("calendar.sources", &cals)
	if len(cals) != 2 || cals[0].Secret == "" || cals[1].Path != "~/work.ics" || cals[1].Color != "#123456" {
		t.Fatalf("calendars: %+v", cals)
	}
	all, _ := st.All()
	for k, v := range all {
		if strings.Contains(string(v), "private-abc") {
			t.Fatalf("calendar link stored as a plain setting in %s", k)
		}
	}
	if link, err := secrets.Get(cals[0].Secret); err != nil || link != "https://cal.example/private-abc/basic.ics" {
		t.Fatalf("link in the secret store: %q %v", link, err)
	}
	// The old files are kept, renamed; a second run finds nothing.
	for _, f := range r.Imported {
		if _, err := os.Stat(f + ".migrated"); err != nil {
			t.Errorf("%s not kept as .migrated", f)
		}
		if _, err := os.Stat(f); err == nil {
			t.Errorf("%s still in place", f)
		}
	}
	if again := Run(st, secrets, d); len(again.Imported) != 0 || len(again.Failed) != 0 {
		t.Fatalf("second run: %+v", again)
	}
}

func TestBrokenFilesStayWhereTheyAre(t *testing.T) {
	root := t.TempDir()
	d := Dirs{Config: root, State: root}
	_ = os.WriteFile(filepath.Join(root, "weather.toml"), []byte("city = "), 0o644)
	_ = os.WriteFile(filepath.Join(root, "displays.toml"), []byte("[[monitor]]\nid = \"bad\"\n"), 0o644)
	_ = os.WriteFile(filepath.Join(root, "dock.json"), []byte(`{"pinned":"not a list"}`), 0o644)
	st, err := store.Open(filepath.Join(root, "settings.db"))
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()
	r := Run(st, &store.Resolver{Fallback: st.Fallback()}, d)
	if len(r.Imported) != 0 || len(r.Failed) != 3 {
		t.Fatalf("%+v", r)
	}
	for f := range r.Failed {
		if _, err := os.Stat(f); err != nil {
			t.Errorf("%s was moved despite failing", f)
		}
	}
}
