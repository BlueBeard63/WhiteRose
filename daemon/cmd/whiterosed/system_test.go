package main

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"testing/fstest"
	"whiterose/daemon/internal/session"
)

func TestReadState(t *testing.T) {
	bl := fstest.MapFS{
		"intel_backlight/type":           {Data: []byte("raw\n")},
		"intel_backlight/brightness":     {Data: []byte("236\n")},
		"intel_backlight/max_brightness": {Data: []byte("400\n")},
	}
	rf := fstest.MapFS{
		"rfkill0/type": {Data: []byte("wlan")}, "rfkill0/soft": {Data: []byte("1")}, "rfkill0/hard": {Data: []byte("0")},
		"rfkill1/type": {Data: []byte("bluetooth")}, "rfkill1/soft": {Data: []byte("1")}, "rfkill1/hard": {Data: []byte("0")},
	}
	s := readState(bl, rf)
	if s.Brightness == nil || *s.Brightness != 59 || s.Backlight != "intel_backlight" || !s.Airplane || s.Radios != 2 {
		t.Fatalf("got %+v", s)
	}
	none := readState(fstest.MapFS{}, fstest.MapFS{})
	if none.Brightness != nil || none.Airplane {
		t.Fatalf("no devices: got %+v", none)
	}
}

func TestEmitChangesSkipsRepeats(t *testing.T) {
	states := []SystemState{{Radios: 1}, {Radios: 1}, {Radios: 1, Airplane: true}, {Radios: 1, Airplane: true}}
	i := 0
	read := func() SystemState { s := states[i]; i++; return s }
	ch := make(chan struct{}, 3)
	ch <- struct{}{}
	ch <- struct{}{}
	ch <- struct{}{}
	close(ch)
	var out bytes.Buffer
	if err := emitChanges(ch, read, &out); err != nil {
		t.Fatal(err)
	}
	want := `{"brightness":null,"airplane":false,"radios":1,"weather":null,"calendars":0}` + "\n" + `{"brightness":null,"airplane":true,"radios":1,"weather":null,"calendars":0}` + "\n"
	if out.String() != want {
		t.Fatalf("got\n%s\nwant\n%s", out.String(), want)
	}
}

func TestBrightnessAndAirplaneRejectBadArguments(t *testing.T) {
	for _, args := range [][]string{{"brightness"}, {"brightness", "set", "0"}, {"brightness", "set", "101"}, {"brightness", "set", "x"}, {"airplane"}, {"airplane", "maybe"}} {
		var out, errb bytes.Buffer
		if code := run(args, &out, &errb); code != 2 || !strings.Contains(errb.String(), "whiterosed") && !strings.Contains(errb.String(), "usage") {
			t.Errorf("%v: exit %d, stderr %q", args, code, errb.String())
		}
	}
}

func TestLockLine(t *testing.T) {
	if got := lockLine(session.Lock); got != `{"event":"lock"}`+"\n" {
		t.Fatalf("got %q", got)
	}
}

func TestCalendarEventsCommand(t *testing.T) {
	root := t.TempDir()
	t.Setenv("XDG_CONFIG_HOME", filepath.Join(root, "config"))
	t.Setenv("XDG_DATA_HOME", filepath.Join(root, "data"))
	t.Setenv("XDG_CACHE_HOME", filepath.Join(root, "cache"))
	ics := "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:1\r\nSUMMARY:Gym\r\nDTSTART:20261109T183000\r\nDTEND:20261109T193000\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
	p := filepath.Join(root, "data", "whiterose", "calendars", "Personal.ics")
	if err := os.MkdirAll(filepath.Dir(p), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(p, []byte(ics), 0o644); err != nil {
		t.Fatal(err)
	}
	var out, errb bytes.Buffer
	if code := run([]string{"calendar", "events", "--from", "2026-11-09", "--to", "2026-11-10"}, &out, &errb); code != 0 {
		t.Fatalf("exit %d: %s", code, errb.String())
	}
	var evs []map[string]any
	if err := json.Unmarshal(out.Bytes(), &evs); err != nil || len(evs) != 1 || evs[0]["title"] != "Gym" || evs[0]["calendar"] != "Personal" {
		t.Fatalf("got %s (%v)", out.String(), err)
	}
	for _, bad := range [][]string{{"calendar"}, {"calendar", "events", "--from", "x", "--to", "2026-11-10"}, {"calendar", "events", "--from", "2026-11-10", "--to", "2026-11-09"}, {"calendar", "nope"}} {
		if code := run(bad, &out, &errb); code != 2 {
			t.Errorf("%v: exit %d", bad, code)
		}
	}
}
