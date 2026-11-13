// Package portal is WhiteRose's xdg-desktop-portal backend
// (org.freedesktop.impl.portal.desktop.whiterose): the FileChooser, so an
// app's Open and Save dialogs are WhiteRose Files (`whiterose-files
// --choose`, apps/files/src/choose.rs). `whiterosed portal` serves it; D-Bus
// starts it when xdg-desktop-portal first asks (whiterose-portals.conf
// sends FileChooser here, the rest to GTK's).
//
// Each request runs a picker; its Request object's Close (the app gave up)
// ends it. Choices (the extra check boxes some apps add) aren't shown.
package portal

import (
	"bytes"
	"encoding/json"
	"errors"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync"

	"github.com/godbus/dbus/v5"
	"github.com/godbus/dbus/v5/introspect"
)

const (
	BusName   = "org.freedesktop.impl.portal.desktop.whiterose"
	Path      = "/org/freedesktop/portal/desktop"
	Interface = "org.freedesktop.impl.portal.FileChooser"
	request   = "org.freedesktop.impl.portal.Request"
)

// Responses.
const (
	Success   uint32 = 0
	Cancelled uint32 = 1
	Failed    uint32 = 2
)

// Filter is the portal's (sa(us)): a name and (kind, pattern) pairs, kind 0
// a glob and 1 a MIME type.
type Filter struct {
	Name     string
	Patterns []struct {
		Kind    uint32
		Pattern string
	}
}

func filterArg(f Filter) (string, bool) {
	var pats []string
	for _, p := range f.Patterns {
		if p.Pattern != "" && !strings.ContainsAny(p.Pattern, " \t") {
			pats = append(pats, p.Pattern)
		}
	}
	if len(pats) == 0 {
		return "", false
	}
	// (Files splits NAME:PATTERNS at the first colon.)
	return strings.ReplaceAll(f.Name, ":", " -") + ":" + strings.Join(pats, " "), true
}

func bytesPath(v dbus.Variant) string {
	b, ok := v.Value().([]byte)
	if !ok {
		return ""
	}
	return string(bytes.TrimRight(b, "\x00"))
}

// Args are whiterose-files' arguments for a request: `save` for SaveFile,
// `folderOnly` for SaveFiles (where the files go).
func Args(title string, opts map[string]dbus.Variant, save, folderOnly bool) []string {
	a := []string{"--choose", "--json"}
	if title != "" {
		a = append(a, "--title", title)
	}
	var b bool
	switch {
	case folderOnly:
		a = append(a, "--folder")
	case save:
		name := ""
		if v, ok := opts["current_name"]; ok {
			name, _ = v.Value().(string)
		}
		if v, ok := opts["current_file"]; ok {
			if f := bytesPath(v); f != "" {
				name = filepath.Base(f)
				if _, ok := opts["current_folder"]; !ok {
					opts["current_folder"] = dbus.MakeVariant([]byte(filepath.Dir(f)))
				}
			}
		}
		if name == "" {
			name = "Untitled"
		}
		a = append(a, "--save", name)
	default:
		if v, ok := opts["directory"]; ok && v.Store(&b) == nil && b {
			a = append(a, "--folder")
		}
		if v, ok := opts["multiple"]; ok && v.Store(&b) == nil && b {
			a = append(a, "--multiple")
		}
	}
	if v, ok := opts["current_folder"]; ok {
		if dir := bytesPath(v); dir != "" {
			a = append(a, "--in", dir)
		}
	}
	if folderOnly {
		return a
	}
	var filters []Filter
	if v, ok := opts["filters"]; ok {
		_ = v.Store(&filters)
	}
	// The one the app wants shown goes first.
	if v, ok := opts["current_filter"]; ok {
		var cur Filter
		if v.Store(&cur) == nil && cur.Name != "" {
			rest := []Filter{cur}
			for _, f := range filters {
				if f.Name != cur.Name {
					rest = append(rest, f)
				}
			}
			filters = rest
		}
	}
	for _, f := range filters {
		if arg, ok := filterArg(f); ok {
			a = append(a, "--filter", arg)
		}
	}
	return a
}

// URIs are paths as file:// URIs.
func URIs(paths []string) []string {
	out := make([]string, len(paths))
	for i, p := range paths {
		out[i] = (&url.URL{Scheme: "file", Path: p}).String()
	}
	return out
}

// FileChooser answers the portal's calls.
type FileChooser struct {
	Conn *dbus.Conn
	// Files is the whiterose-files command.
	Files string
	Log   func(format string, args ...any)

	mu      sync.Mutex
	running map[dbus.ObjectPath]*exec.Cmd
}

// Request is the object at a request's handle: Close ends its picker.
type Request struct {
	fc     *FileChooser
	handle dbus.ObjectPath
}

func (r *Request) Close() *dbus.Error {
	r.fc.mu.Lock()
	cmd := r.fc.running[r.handle]
	r.fc.mu.Unlock()
	if cmd != nil && cmd.Process != nil {
		_ = cmd.Process.Kill()
	}
	return nil
}

// pick runs a picker for `handle`: its paths, or the response it ended with.
func (fc *FileChooser) pick(handle dbus.ObjectPath, args []string) ([]string, uint32) {
	cmd := exec.Command(fc.Files, args...)
	cmd.Stderr = os.Stderr
	var out bytes.Buffer
	cmd.Stdout = &out
	if err := cmd.Start(); err != nil {
		fc.Log("file chooser: %v", err)
		return nil, Failed
	}
	fc.mu.Lock()
	if fc.running == nil {
		fc.running = map[dbus.ObjectPath]*exec.Cmd{}
	}
	fc.running[handle] = cmd
	fc.mu.Unlock()
	if fc.Conn != nil {
		_ = fc.Conn.Export(&Request{fc: fc, handle: handle}, handle, request)
	}
	err := cmd.Wait()
	fc.mu.Lock()
	delete(fc.running, handle)
	fc.mu.Unlock()
	if fc.Conn != nil {
		_ = fc.Conn.Export(nil, handle, request)
	}
	var exit *exec.ExitError
	switch {
	case errors.As(err, &exit) && exit.ExitCode() == 1, errors.As(err, &exit) && exit.ExitCode() == -1:
		return nil, Cancelled // (or Close killed it)
	case err != nil:
		fc.Log("file chooser: %v", err)
		return nil, Failed
	}
	var paths []string
	if json.Unmarshal(out.Bytes(), &paths) != nil || len(paths) == 0 {
		return nil, Cancelled
	}
	return paths, Success
}

func answer(paths []string, code uint32) (uint32, map[string]dbus.Variant, *dbus.Error) {
	res := map[string]dbus.Variant{}
	if code == Success {
		res["uris"] = dbus.MakeVariant(URIs(paths))
	}
	return code, res, nil
}

func (fc *FileChooser) OpenFile(handle dbus.ObjectPath, appID, parent, title string, opts map[string]dbus.Variant) (uint32, map[string]dbus.Variant, *dbus.Error) {
	return answer(fc.pick(handle, Args(title, opts, false, false)))
}

func (fc *FileChooser) SaveFile(handle dbus.ObjectPath, appID, parent, title string, opts map[string]dbus.Variant) (uint32, map[string]dbus.Variant, *dbus.Error) {
	return answer(fc.pick(handle, Args(title, opts, true, false)))
}

// SaveFiles: a folder for several files (opts "files": their names).
func (fc *FileChooser) SaveFiles(handle dbus.ObjectPath, appID, parent, title string, opts map[string]dbus.Variant) (uint32, map[string]dbus.Variant, *dbus.Error) {
	paths, code := fc.pick(handle, Args(title, opts, false, true))
	if code != Success {
		return answer(nil, code)
	}
	var names [][]byte
	if v, ok := opts["files"]; ok {
		_ = v.Store(&names)
	}
	var out []string
	for _, n := range names {
		out = append(out, filepath.Join(paths[0], filepath.Base(string(bytes.TrimRight(n, "\x00")))))
	}
	return answer(out, Success)
}

const introspection = `<interface name="` + Interface + `">
  <method name="OpenFile"><arg type="o" name="handle" direction="in"/><arg type="s" name="app_id" direction="in"/><arg type="s" name="parent_window" direction="in"/><arg type="s" name="title" direction="in"/><arg type="a{sv}" name="options" direction="in"/><arg type="u" name="response" direction="out"/><arg type="a{sv}" name="results" direction="out"/></method>
  <method name="SaveFile"><arg type="o" name="handle" direction="in"/><arg type="s" name="app_id" direction="in"/><arg type="s" name="parent_window" direction="in"/><arg type="s" name="title" direction="in"/><arg type="a{sv}" name="options" direction="in"/><arg type="u" name="response" direction="out"/><arg type="a{sv}" name="results" direction="out"/></method>
  <method name="SaveFiles"><arg type="o" name="handle" direction="in"/><arg type="s" name="app_id" direction="in"/><arg type="s" name="parent_window" direction="in"/><arg type="s" name="title" direction="in"/><arg type="a{sv}" name="options" direction="in"/><arg type="u" name="response" direction="out"/><arg type="a{sv}" name="results" direction="out"/></method>
</interface>`

// Serve exports the FileChooser on `conn` and takes the backend's name.
func Serve(conn *dbus.Conn, fc *FileChooser) error {
	fc.Conn = conn
	if err := conn.Export(fc, Path, Interface); err != nil {
		return err
	}
	_ = conn.Export(introspect.Introspectable(`<node>`+introspection+introspect.IntrospectDataString+`</node>`), Path, "org.freedesktop.DBus.Introspectable")
	reply, err := conn.RequestName(BusName, dbus.NameFlagDoNotQueue)
	if err != nil {
		return err
	}
	if reply != dbus.RequestNameReplyPrimaryOwner {
		return errors.New(BusName + " is already taken")
	}
	return nil
}
