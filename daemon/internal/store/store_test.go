package store

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"
)

func open(t *testing.T) (*Store, string) {
	t.Helper()
	p := filepath.Join(t.TempDir(), "whiterose", "settings.db")
	s, err := Open(p)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { s.Close() })
	return s, p
}

func TestDefaultsSetGetAndPersistence(t *testing.T) {
	s, p := open(t)
	v, ok, err := s.Get("dock.pinned")
	if err != nil || !ok || !strings.Contains(string(v), "org.gnome.Nautilus") {
		t.Fatalf("default dock: %s %v %v", v, ok, err)
	}
	if err := s.Set("dock.pinned", json.RawMessage(` [ "firefox_firefox" ] `)); err != nil {
		t.Fatal(err)
	}
	if err := s.SetValue("quick.doNotDisturb", true); err != nil {
		t.Fatal(err)
	}
	s.Close()
	s2, err := Open(p)
	if err != nil {
		t.Fatal(err)
	}
	defer s2.Close()
	var pinned []string
	var dnd bool
	if err := s2.GetInto("dock.pinned", &pinned); err != nil || len(pinned) != 1 || pinned[0] != "firefox_firefox" {
		t.Fatalf("after reopening: %v %v", pinned, err)
	}
	_ = s2.GetInto("quick.doNotDisturb", &dnd)
	if !dnd {
		t.Fatal("bool setting lost")
	}
	all, _ := s2.All()
	if string(all["dock.pinned"]) != `["firefox_firefox"]` || string(all["weather.units"]) != `"celsius"` {
		t.Fatalf("All mixes stored values and defaults: %s %s", all["dock.pinned"], all["weather.units"])
	}
	if err := s2.Reset("dock.pinned"); err != nil {
		t.Fatal(err)
	}
	_ = s2.GetInto("dock.pinned", &pinned)
	if len(pinned) != 6 {
		t.Fatalf("reset should bring the default back: %v", pinned)
	}
}

func TestOnlyTheUserCanReadTheDatabase(t *testing.T) {
	_, p := open(t)
	for _, f := range []string{p, filepath.Dir(p)} {
		fi, err := os.Stat(f)
		if err != nil {
			t.Fatal(err)
		}
		if fi.Mode().Perm()&0o077 != 0 {
			t.Errorf("%s is %v: others can read it", f, fi.Mode().Perm())
		}
	}
}

func TestValidation(t *testing.T) {
	s, _ := open(t)
	bad := map[string]string{
		"dock.pinned":                   `"firefox"`,
		"quick.doNotDisturb":            `"yes"`,
		"quick.nightLight.temperature":  `9000`,
		"weather.units":                 `"kelvin"`,
		"weather.latitude":              `123`,
		"launcher.calculations":         `[{"expr":"1+1","result":"2","extra":true}]`,
		"displays":                      `{"monitor":[{"id":"not-an-id"}],"profile":[]}`,
		"displays.test":                 `{"config":{"monitor":[],"profile":[]}}`,
		"keyboard.layouts":              `[{"layout":"gb; rm -rf"}]`,
		"keyboard.repeatDelay":          `20`,
		"keyboard.repeatRate":           `500`,
		"shortcuts.keys":                `{"close-window":["SUPER++Q"]}`,
		"shortcuts.custom":              `[{"id":"files","name":"Files","command":"nautilus","keys":["SUPER+E"]}]`,
		"appearance.theme":              `"sepia"`,
		"appearance.accent":             `"pink"`,
		"appearance.size":               `"huge"`,
		"wallpaper.folders":             `["relative/dir"]`,
		"wallpaper.rotateMinutes":       `7`,
		"bar.timeFormat":                `"13h"`,
		"bar.screens":                   `"some"`,
		"bar.seconds":                   `"yes"`,
		"privacy.lockAfter":             `7`,
		"privacy.screenOffAfter":        `-1`,
		"apps.terminal":                 `"ghostty --title x"`,
		"apps.editor":                   `"/usr/bin/code"`,
		"files.keys":                    `{"new-tab":"Ctrl+T; rm"}`,
		"files.startIn":                 `"somewhere"`,
		"files.tags":                    `[{"name":"a,b","colour":"red"}]`,
		"files.startFolder":             `"relative/dir"`,
		"files.zoom":                    `3`,
		"files.columns":                 `["size","size"]`,
		"files.widths":                  `{"size":5}`,
		"files.defaultView":             `{"mode":"huge","sortBy":"name","descending":false,"group":""}`,
		"windows.layout":                `"spiral"`,
		"windows.layout.workspaces":     `{"10":"dwindle"}`,
		"windows.layout.monitors":       `{"DP-1":"master"}`,
		"windows.gaps.inner":            `100`,
		"windows.master.ratio":          `0.95`,
		"windows.master.side":           `"middle"`,
		"windows.dwindle.split":         `"diagonal"`,
		"windows.snap.zones.workspaces": `{"10":{"builtin":[[0,0,1,1]]}}`,
		"windows.rules":                 `[{"workspace":2}]`,
		"windows.scratchpads":           `[{"id":"a","command":""}]`,
		"nope.unknown":                  `1`,
		"Bad Key":                       `1`,
		"module.x":                      `{not json`,
	}
	for k, v := range bad {
		if err := s.Set(k, json.RawMessage(v)); err == nil {
			t.Errorf("%s = %s was accepted", k, v)
		}
	}
	good := map[string]string{
		"weather.city":                  `"London"`,
		"weather.latitude":              `51.5`,
		"module.wallpaper.path":         `"/home/me/bg.png"`,
		"launcher.calculations":         `[{"expr":"1+1","result":"2"}]`,
		"displays":                      `{"monitor":[],"profile":[]}`,
		"keyboard.layouts":              `[{"layout":"gb"},{"layout":"es","variant":"nodeadkeys"}]`,
		"keyboard.repeatDelay":          `250`,
		"appearance.theme":              `"auto"`,
		"appearance.accent":             `"#8FB8F2"`,
		"wallpaper.current":             `"/home/me/Pictures/sea.jpg"`,
		"wallpaper.folders":             `["~/Pictures/Wallpapers","/usr/share/backgrounds"]`,
		"wallpaper.rotateMinutes":       `30`,
		"shortcuts.keys":                `{"close-window":["SUPER+Q","ALT+F4"],"lock":[],"workspace":["SUPER+1-9"],"focus-direction":["SUPER+ARROWS"]}`,
		"shortcuts.custom":              `[{"id":"custom-files","name":"Open Files","command":"nautilus","keys":["SUPER+E"]}]`,
		"displays.test":                 `{"config":{"monitor":[{"id":"builtin","scale":1.5}],"profile":[]},"until":1794240000000}`,
		"apps.terminal":                 `"ghostty"`,
		"apps.editor":                   `""`,
		"files.keys":                    `{"new-tab":"Ctrl+Shift+Y","copy-other":"F7"}`,
		"files.startIn":                 `"folder"`,
		"files.tags":                    `[{"name":"Work","colour":"blue"},{"name":"Holiday 2026","colour":"teal"}]`,
		"files.startFolder":             `"~/Projects"`,
		"files.zoom":                    `1.2`,
		"files.columns":                 `["kind","created"]`,
		"files.widths":                  `{"kind":140}`,
		"files.defaultView":             `{"mode":"list","sortBy":"modified","descending":true,"group":"modified"}`,
		"files.indexExclude":            `["~/Videos/Raw"]`,
		"windows.layout":                `"dwindle"`,
		"windows.layout.workspaces":     `{"1":"master","9":"floating"}`,
		"windows.layout.monitors":       `{"builtin":"dwindle","edid:0123456789ab":"master"}`,
		"windows.gaps.inner":            `0`,
		"windows.gaps.smart":            `true`,
		"windows.master.ratio":          `0.6`,
		"windows.master.side":           `"bottom"`,
		"windows.dwindle.split":         `"vertical"`,
		"windows.snap":                  `false`,
		"windows.snap.zones":            `{"builtin":[[0,0,0.25,1],[0.25,0,0.5,1],[0.75,0,0.25,1]]}`,
		"windows.snap.zones.workspaces": `{"2":{"builtin":[[0,0,0.5,1]],"edid:0123456789ab":[[0,0,1,0.5],[0,0.5,1,0.5]]}}`,
	}
	for k, v := range good {
		if err := s.Set(k, json.RawMessage(v)); err != nil {
			t.Errorf("%s = %s refused: %v", k, v, err)
		}
	}
	if err := s.Set("module.big", json.RawMessage(`"`+strings.Repeat("x", MaxValue)+`"`)); err == nil {
		t.Error("oversized value accepted")
	}
}

func TestWindowRules(t *testing.T) {
	s, _ := open(t)
	for _, v := range []string{
		`[{"workspace":2}]`,
		`[{"app":"","title":""}]`,
		`[{"app":"("}]`,
		`[{"app":"a","workspace":10}]`,
		`[{"app":"a","size":[0,"50%"]}]`,
		`[{"app":"a","size":[400]}]`,
		`[{"app":"a","position":"middle"}]`,
		`[{"app":"a","position":[10,"120%"]}]`,
		`[{"app":"a","opacity":[1]}]`,
		`[{"app":"a","opacity":[0,1]}]`,
		`[{"app":"a","float":"yes"}]`,
		`[{"app":"a","zone":0}]`,
		`[{"app":"a","blur":true}]`,
		`{"app":"a"}`,
	} {
		if err := s.Set("windows.rules", json.RawMessage(v)); err == nil {
			t.Errorf("windows.rules = %s was accepted", v)
		}
	}
	for _, v := range []string{
		`[]`,
		`[{"app":"^org\\.gnome\\.Ptyxis$","workspace":3,"float":true,"size":[900,"60%"],"position":"centre"}]`,
		`[{"title":"^Picture-in-Picture$","onTop":true,"opacity":[1,0.85],"position":[0,"5%"]}]`,
		`[{"app":"a","monitor":"edid:0123456789ab","maximized":true,"fullscreen":false,"zone":2}]`,
	} {
		if err := s.Set("windows.rules", json.RawMessage(v)); err != nil {
			t.Errorf("windows.rules = %s refused: %v", v, err)
		}
	}
}

func TestScratchpads(t *testing.T) {
	s, _ := open(t)
	for _, v := range []string{
		`[{"id":"a","command":"","keys":["SUPER+GRAVE"]}]`,
		`[{"id":"","command":"foot","keys":["SUPER+GRAVE"]}]`,
		`[{"id":"Bad Id","command":"foot","keys":["SUPER+GRAVE"]}]`,
		`[{"id":"a","command":"foot","keys":["SUPER+GRAVE"]},{"id":"a","command":"kitty","keys":["SUPER+N"]}]`,
		`[{"id":"a","command":"foot","keys":["SUPER+"]}]`,
		`[{"id":"a","command":"foot","keys":["SUPER+GRAVE"],"size":[0,"50%"]}]`,
		`[{"id":"a","command":"foot","keys":["SUPER+GRAVE"],"size":["60%"]}]`,
		`[{"id":"a","command":"foot","keys":["SUPER+GRAVE"],"place":"bottom"}]`,
		`[{"id":"a","command":"foot","keys":["SUPER+GRAVE"],"hideOnBlur":"yes"}]`,
		`[{"id":"a","command":"foot","keys":["SUPER+GRAVE"],"workspace":2}]`,
		`{"id":"a","command":"foot","keys":["SUPER+GRAVE"]}`,
	} {
		if err := s.Set("windows.scratchpads", json.RawMessage(v)); err == nil {
			t.Errorf("windows.scratchpads = %s was accepted", v)
		}
	}
	for _, v := range []string{
		`[]`,
		`[{"id":"a","command":"foot","keys":[]}]`,
		`[{"id":"pad-1","name":"Terminal","command":"ghostty --gtk-single-instance=false","keys":["SUPER+GRAVE"]}]`,
		`[{"id":"notes","name":"Notes","command":"whiterose-notepad","keys":["SUPER+N","SUPER+SHIFT+N"],"size":[900,"40%"],"place":"centre","hideOnBlur":true}]`,
	} {
		if err := s.Set("windows.scratchpads", json.RawMessage(v)); err != nil {
			t.Errorf("windows.scratchpads = %s refused: %v", v, err)
		}
	}
}

func TestSubscribersSeeEveryChange(t *testing.T) {
	s, _ := open(t)
	ch, stop := s.Subscribe()
	defer stop()
	_ = s.SetValue("weather.city", "Leeds")
	_ = s.Reset("weather.city")
	for _, want := range []string{`"Leeds"`, `""`} {
		select {
		case c := <-ch:
			if c.Key != "weather.city" || string(c.Value) != want {
				t.Fatalf("got %s=%s, want %s", c.Key, c.Value, want)
			}
		case <-time.After(time.Second):
			t.Fatal("no change delivered")
		}
	}
	stop()
	stop() // safe twice
	_ = s.SetValue("weather.city", "York")
}

func TestConcurrentWritersAndReaders(t *testing.T) {
	s, _ := open(t)
	var wg sync.WaitGroup
	for i := 0; i < 8; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			for j := 0; j < 20; j++ {
				if err := s.SetValue("module.test.n", i*100+j); err != nil {
					t.Error(err)
				}
				if _, err := s.All(); err != nil {
					t.Error(err)
				}
			}
		}(i)
	}
	wg.Wait()
}

func TestNewerSchemaIsRefused(t *testing.T) {
	s, p := open(t)
	if _, err := s.db.Exec(`PRAGMA user_version = 99`); err != nil {
		t.Fatal(err)
	}
	s.Close()
	if _, err := Open(p); err == nil || !strings.Contains(err.Error(), "newer") {
		t.Fatalf("a database from a newer WhiteRose must not be touched: %v", err)
	}
}

// fakeKeyring is an in-memory Secret Service.
type fakeKeyring struct {
	m    map[string]string
	fail bool
}

func (f *fakeKeyring) Name() string { return "keyring" }
func (f *fakeKeyring) Get(id string) (string, error) {
	v, ok := f.m[id]
	if !ok {
		return "", ErrNoSecret
	}
	return v, nil
}
func (f *fakeKeyring) Set(id, v string) error {
	if f.fail {
		return errors.New("locked")
	}
	f.m[id] = v
	return nil
}
func (f *fakeKeyring) Delete(id string) error { delete(f.m, id); return nil }

func TestSecretsPreferTheKeyringAndMoveThere(t *testing.T) {
	s, _ := open(t)
	fb := s.Fallback()
	// No keyring yet: secrets go in the database.
	r := &Resolver{Fallback: fb}
	id := NewSecretID()
	if where, err := r.Set(id, "https://cal.example/private-abc"); err != nil || where != "settings database" {
		t.Fatalf("%s %v", where, err)
	}
	// A keyring appears (next login): the secret moves there.
	k := &fakeKeyring{m: map[string]string{}}
	if n, err := s.MoveToKeyring(k); err != nil || n != 1 {
		t.Fatalf("moved %d: %v", n, err)
	}
	if _, err := fb.Get(id); !errors.Is(err, ErrNoSecret) {
		t.Fatal("secret left in the database after moving")
	}
	r = &Resolver{Keyring: k, Fallback: fb}
	if v, err := r.Get(id); err != nil || v != "https://cal.example/private-abc" {
		t.Fatalf("%q %v", v, err)
	}
	// A keyring that refuses (locked): fall back rather than lose it.
	k.fail = true
	id2 := NewSecretID()
	if where, err := r.Set(id2, "x"); err != nil || where != "settings database" {
		t.Fatalf("%s %v", where, err)
	}
	if v, _ := r.Get(id2); v != "x" {
		t.Fatal("fallback secret not readable")
	}
	r.Delete(id)
	r.Delete(id2)
	if _, err := r.Get(id); err == nil {
		t.Fatal("deleted secret still readable")
	}
	if NewSecretID() == NewSecretID() {
		t.Fatal("secret ids must be unique")
	}
}

func TestSnapZoneGridsAreChecked(t *testing.T) {
	s, _ := open(t)
	for _, v := range []string{
		`{"builtin":[]}`, // no zones
		`{"builtin":[[0,0,0.6,1],[0.5,0,0.5,1]]}`,        // overlap
		`{"builtin":[[0.6,0,0.5,1]]}`,                    // outside
		`{"builtin":[[0,0,0.05,1]]}`,                     // too small
		`{"builtin":[[0,0,1]]}`,                          // not four numbers
		`{"DP 1!":[[0,0,1,1]]}`,                          // not a monitor id
		`{"builtin":[[0,0,0.5,1],[0.5,0,0.5,1]],"x":[]}`, // one bad grid
	} {
		if err := s.Set("windows.snap.zones", json.RawMessage(v)); err == nil {
			t.Errorf("%s was accepted", v)
		}
	}
	// Gaps in a grid are fine.
	if err := s.Set("windows.snap.zones", json.RawMessage(`{"builtin":[[0,0,0.4,1],[0.6,0,0.4,1]]}`)); err != nil {
		t.Error(err)
	}
}

// A keyring that's locked at the start (a fingerprint login) is tried
// again: a wanted secret says it's locked rather than missing, and once the
// keyring opens it's used, and Opened runs once.
func TestResolverOpensTheKeyringLate(t *testing.T) {
	s, _ := open(t)
	k := &fakeKeyring{m: map[string]string{"cal": "https://cal.example/x"}}
	locked := true
	tries, opened := 0, 0
	r := &Resolver{Fallback: s.Fallback(), Open: func() (Secrets, error) {
		tries++
		if locked {
			return nil, ErrLocked
		}
		return k, nil
	}}
	r.Opened = func(Secrets) { opened++ }
	if r.Retry() {
		t.Fatal("opened a locked keyring")
	}
	if _, err := r.Get("cal"); !errors.Is(err, ErrLocked) {
		t.Fatalf("want ErrLocked, got %v", err)
	}
	if _, err := r.Get("other"); !errors.Is(err, ErrLocked) {
		t.Fatalf("want ErrLocked, got %v", err)
	}
	if !errors.Is(r.Closed(), ErrLocked) {
		t.Fatalf("closed: %v", r.Closed())
	}
	// Gets in between don't each knock on the keyring.
	if tries != 1 {
		t.Fatalf("tried %d times", tries)
	}
	locked = false
	if !r.Retry() {
		t.Fatal("didn't open once unlocked")
	}
	if v, err := r.Get("cal"); err != nil || v != "https://cal.example/x" {
		t.Fatalf("%q %v", v, err)
	}
	r.Retry()
	if opened != 1 || r.Closed() != nil {
		t.Fatalf("opened %d times, closed: %v", opened, r.Closed())
	}
	// No keyring at all (none running): missing is missing.
	r = &Resolver{Fallback: s.Fallback(), Open: func() (Secrets, error) { return nil, errors.New("no keyring running") }}
	if _, err := r.Get("cal"); !errors.Is(err, ErrNoSecret) {
		t.Fatalf("want ErrNoSecret, got %v", err)
	}
}

// What the keyring already has stays: a copy kept in the database while it
// was locked doesn't replace it (Files' key: what it sealed needs the
// original).
func TestMoveToKeyringKeepsTheKeyringsOwn(t *testing.T) {
	s, _ := open(t)
	fb := s.Fallback()
	_ = fb.Set("files:seal-key", "made-while-locked")
	_ = fb.Set("cal", "https://cal.example/new")
	k := &fakeKeyring{m: map[string]string{"files:seal-key": "original"}}
	if n, err := s.MoveToKeyring(k); err != nil || n != 1 {
		t.Fatalf("moved %d: %v", n, err)
	}
	if k.m["files:seal-key"] != "original" || k.m["cal"] != "https://cal.example/new" {
		t.Fatalf("keyring: %v", k.m)
	}
	if _, err := fb.Get("files:seal-key"); !errors.Is(err, ErrNoSecret) {
		t.Fatal("the database copy is left behind")
	}
}
