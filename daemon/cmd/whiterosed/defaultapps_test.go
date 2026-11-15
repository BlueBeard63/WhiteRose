package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"whiterose/daemon/internal/server"
)

func TestDefaultAppsOps(t *testing.T) {
	r := t.TempDir()
	t.Setenv("XDG_CONFIG_HOME", filepath.Join(r, "config"))
	t.Setenv("XDG_CONFIG_DIRS", filepath.Join(r, "etc"))
	t.Setenv("XDG_DATA_HOME", filepath.Join(r, "data"))
	t.Setenv("XDG_DATA_DIRS", filepath.Join(r, "usr"))
	w := func(p, s string) { os.MkdirAll(filepath.Dir(p), 0o755); os.WriteFile(p, []byte(s), 0o644) }
	app := func(name, types string) string {
		return "[Desktop Entry]\nType=Application\nName=" + name + "\nIcon=x\nMimeType=" + types + "\n"
	}
	w(filepath.Join(r, "usr/applications/libreoffice-calc.desktop"), app("LibreOffice Calc", "text/csv;application/vnd.ms-excel;"))
	w(filepath.Join(r, "usr/applications/org.gnome.TextEditor.desktop"), app("Text Editor", "text/plain;text/csv;"))
	w(filepath.Join(r, "etc/whiterose-mimeapps.list"), "[Default Applications]\ntext/csv=libreoffice-calc.desktop;\n")
	w(filepath.Join(r, "usr/mime/globs2"), "50:text/csv:*.csv\n")
	ops := defaultAppsOps()

	get := func() map[string]any {
		v, err := ops["default_apps"](server.Request{})
		if err != nil {
			t.Fatal(err)
		}
		b, _ := json.Marshal(v)
		var m map[string]any
		json.Unmarshal(b, &m)
		return m
	}
	cat := func(m map[string]any, key string) map[string]any {
		for _, c := range m["categories"].([]any) {
			if c.(map[string]any)["key"] == key {
				return c.(map[string]any)
			}
		}
		t.Fatalf("no %s", key)
		return nil
	}
	sheets := cat(get(), "spreadsheets")
	if sheets["current"].(map[string]any)["id"] != "libreoffice-calc.desktop" || len(sheets["apps"].([]any)) != 2 || sheets["changed"] != false {
		t.Errorf("%v", sheets)
	}
	// Set by extension search, then the category shows yours.
	found, _ := ops["default_apps_search"](server.Request{Name: ".csv"})
	if b, _ := json.Marshal(found); !strings.Contains(string(b), `"mime":"text/csv"`) {
		t.Fatalf("%s", b)
	}
	if _, err := ops["default_apps_set"](server.Request{Key: "type:text/csv", Name: "org.gnome.TextEditor.desktop"}); err != nil {
		t.Fatal(err)
	}
	m := get()
	if cat(m, "spreadsheets")["changed"] != true || len(m["changed"].([]any)) != 1 {
		t.Errorf("%v", m)
	}
	// Refused: an app that isn't there, a made-up kind, a type that would
	// break the file.
	for _, req := range []server.Request{{Key: "spreadsheets", Name: "nope.desktop"}, {Key: "nonsense", Name: "org.gnome.TextEditor.desktop"}, {Key: "type:a=b\n[x]", Name: "org.gnome.TextEditor.desktop"}} {
		if _, err := ops["default_apps_set"](req); err == nil {
			t.Errorf("accepted %+v", req)
		}
	}
	if _, err := ops["default_apps_reset"](server.Request{Key: "spreadsheets"}); err != nil {
		t.Fatal(err)
	}
	if b, _ := os.ReadFile(filepath.Join(r, "config/whiterose-mimeapps.list")); strings.Contains(string(b), "text/csv") {
		t.Errorf("not reset: %s", b)
	}
}
