package main

import (
	"bytes"
	"encoding/json"
	"net"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"testing/fstest"
	"whiterose/daemon/internal/calendar"
	"whiterose/daemon/internal/server"
	"whiterose/daemon/internal/store"
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

func TestBrightnessAndAirplaneRejectBadArguments(t *testing.T) {
	for _, args := range [][]string{{"brightness"}, {"brightness", "set", "0"}, {"brightness", "set", "101"}, {"brightness", "set", "x"}, {"airplane"}, {"airplane", "maybe"}} {
		var out, errb bytes.Buffer
		if code := run(args, &out, &errb); code != 2 || !strings.Contains(errb.String(), "whiterosed") && !strings.Contains(errb.String(), "usage") {
			t.Errorf("%v: exit %d, stderr %q", args, code, errb.String())
		}
	}
}

func TestSettingsAndCalendarCommandsTalkToTheDaemon(t *testing.T) {
	root := t.TempDir()
	st, err := store.Open(filepath.Join(root, "settings.db"))
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()
	d := calendar.Dirs{Data: filepath.Join(root, "data"), Cache: filepath.Join(root, "cache"), Home: root}
	srv := &server.Server{Store: st, Secrets: store.Resolver{Fallback: st.Fallback()}, Dirs: d}
	sock := filepath.Join(root, "d.sock")
	ln, err := net.Listen("unix", sock)
	if err != nil {
		t.Fatal(err)
	}
	defer ln.Close()
	go srv.Serve(ln)
	t.Setenv("WHITEROSE_DAEMON_SOCKET", sock)

	cli := func(args ...string) (string, int) {
		var out, errb bytes.Buffer
		code := run(args, &out, &errb)
		return out.String() + errb.String(), code
	}
	if out, code := cli("settings", "set", "weather.city", `"London"`); code != 0 {
		t.Fatalf("set: %s", out)
	}
	if out, code := cli("settings", "get", "weather.city"); code != 0 || strings.TrimSpace(out) != `"London"` {
		t.Fatalf("get: %q", out)
	}
	if out, code := cli("settings", "list"); code != 0 || !strings.Contains(out, `weather.city = "London"`) || !strings.Contains(out, "dock.pinned = ") {
		t.Fatalf("list: %s", out)
	}
	if out, code := cli("settings", "set", "weather.city", "London"); code != 2 || !strings.Contains(out, "JSON") {
		t.Fatalf("unquoted word: %d %s", code, out)
	}
	if out, code := cli("settings", "set", "quick.doNotDisturb", `"maybe"`); code != 1 {
		t.Fatalf("invalid value: %d %s", code, out)
	}
	ics := "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:1\r\nSUMMARY:Gym\r\nDTSTART:20261109T183000\r\nDTEND:20261109T193000\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
	_ = os.WriteFile(filepath.Join(root, "gym.ics"), []byte(ics), 0o644)
	if out, code := cli("calendar", "add", "Gym", filepath.Join(root, "gym.ics"), "#9BD5A8"); code != 0 {
		t.Fatalf("add: %s", out)
	}
	if out, code := cli("calendar", "add", "Personal", "https://cal.example/private-xyz/basic.ics"); code != 0 {
		t.Fatalf("add link: %s", out)
	}
	out, code := cli("calendar", "list")
	if code != 0 || !strings.Contains(out, "1. Gym  #9BD5A8") || !strings.Contains(out, "2. Personal") || strings.Contains(out, "private-xyz") {
		t.Fatalf("list: %s", out)
	}
	var evs []map[string]any
	out, _ = cli("calendar", "events", "2026-11-09", "2026-11-10")
	if err := json.Unmarshal([]byte(out), &evs); err != nil || len(evs) != 1 || evs[0]["title"] != "Gym" {
		t.Fatalf("events: %s", out)
	}
	if out, code := cli("calendar", "remove", "2"); code != 0 {
		t.Fatalf("remove: %s", out)
	}
	if _, code := cli("calendar", "remove", "zero"); code != 2 {
		t.Fatal("bad number accepted")
	}

	t.Setenv("WHITEROSE_DAEMON_SOCKET", filepath.Join(root, "nobody.sock"))
	if out, code := cli("settings", "list"); code != 1 || !strings.Contains(out, "isn't running") {
		t.Fatalf("no daemon: %d %s", code, out)
	}
}
