package portal

import (
	"os"
	"path/filepath"
	"reflect"
	"testing"

	"github.com/godbus/dbus/v5"
)

type pat = struct {
	Kind    uint32
	Pattern string
}

func TestArgsFromPortalOptions(t *testing.T) {
	images := Filter{Name: "Images", Patterns: []pat{{0, "*.png"}, {1, "image/jpeg"}}}
	text := Filter{Name: "Text: plain", Patterns: []pat{{0, "*.txt"}}}
	got := Args("Open Image", map[string]dbus.Variant{
		"multiple":       dbus.MakeVariant(true),
		"current_folder": dbus.MakeVariant([]byte("/home/me/Pictures\x00")),
		"filters":        dbus.MakeVariant([]Filter{text, images}),
		"current_filter": dbus.MakeVariant(images),
	}, false, false)
	want := []string{"--choose", "--json", "--title", "Open Image", "--multiple", "--in", "/home/me/Pictures",
		"--filter", "Images:*.png image/jpeg", "--filter", "Text - plain:*.txt"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("open:\n got %q\nwant %q", got, want)
	}

	got = Args("", map[string]dbus.Variant{"directory": dbus.MakeVariant(true)}, false, false)
	if !reflect.DeepEqual(got, []string{"--choose", "--json", "--folder"}) {
		t.Errorf("folder: %q", got)
	}

	// Saving: the name from current_file (and its folder), else current_name.
	got = Args("Save", map[string]dbus.Variant{"current_file": dbus.MakeVariant([]byte("/tmp/x/report.pdf\x00"))}, true, false)
	want = []string{"--choose", "--json", "--title", "Save", "--save", "report.pdf", "--in", "/tmp/x"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("save:\n got %q\nwant %q", got, want)
	}
	got = Args("", map[string]dbus.Variant{"current_name": dbus.MakeVariant("a.txt")}, true, false)
	if !reflect.DeepEqual(got, []string{"--choose", "--json", "--save", "a.txt"}) {
		t.Errorf("save name: %q", got)
	}
	got = Args("", map[string]dbus.Variant{"filters": dbus.MakeVariant([]Filter{text})}, false, true)
	if !reflect.DeepEqual(got, []string{"--choose", "--json", "--folder"}) {
		t.Errorf("save files: %q", got)
	}
}

func TestURIs(t *testing.T) {
	got := URIs([]string{"/home/me/a b#1.png"})
	if got[0] != "file:///home/me/a%20b%231.png" {
		t.Errorf("%q", got[0])
	}
}

// A stand-in picker: prints what it's given to print, or exits 1.
func fake(t *testing.T, script string) string {
	p := filepath.Join(t.TempDir(), "whiterose-files")
	os.WriteFile(p, []byte("#!/bin/sh\n"+script+"\n"), 0o755)
	return p
}

func TestRequestsAnswer(t *testing.T) {
	fc := &FileChooser{Files: fake(t, `echo '["/a/b.png"]'`), Log: t.Logf}
	code, res, _ := fc.OpenFile("/r/1", "app", "", "Open", map[string]dbus.Variant{})
	if code != Success || !reflect.DeepEqual(res["uris"].Value(), []string{"file:///a/b.png"}) {
		t.Errorf("open: %d %v", code, res)
	}
	fc.Files = fake(t, `exit 1`)
	if code, res, _ := fc.SaveFile("/r/2", "app", "", "Save", map[string]dbus.Variant{}); code != Cancelled || len(res) != 0 {
		t.Errorf("cancel: %d %v", code, res)
	}
	fc.Files = fake(t, `exit 3`)
	if code, _, _ := fc.OpenFile("/r/3", "app", "", "", map[string]dbus.Variant{}); code != Failed {
		t.Errorf("broken: %d", code)
	}
	fc.Files = fake(t, `echo '["/out"]'`)
	code, res, _ = fc.SaveFiles("/r/4", "app", "", "", map[string]dbus.Variant{"files": dbus.MakeVariant([][]byte{[]byte("a.txt\x00"), []byte("b.txt\x00")})})
	if code != Success || !reflect.DeepEqual(res["uris"].Value(), []string{"file:///out/a.txt", "file:///out/b.txt"}) {
		t.Errorf("save files: %d %v", code, res)
	}
}
