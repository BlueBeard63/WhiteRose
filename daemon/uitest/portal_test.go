//go:build uitest

// The desktop portal's file chooser (whiterosed portal): an app's Open
// dialog is Files' picker.
package uitest_test

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/godbus/dbus/v5"

	"whiterose/daemon/internal/portal"
	"whiterose/daemon/internal/uitest"
)

type portalPattern = struct {
	Kind    uint32
	Pattern string
}

func TestPortalOpenFileIsFilesPicker(t *testing.T) {
	s := uitest.Start(t, uitest.Options{})
	pics := filepath.Join(s.Home, "Pictures")
	for _, n := range []string{"Shot.PNG", "notes.txt"} {
		os.WriteFile(filepath.Join(pics, n), []byte("x"), 0o644)
	}
	s.Spawn("whiterosed portal")
	conn, err := dbus.Connect(s.Bus())
	if err != nil {
		t.Fatal(err)
	}
	defer conn.Close()
	for i := 0; ; i++ {
		var owned bool
		conn.BusObject().Call("org.freedesktop.DBus.NameHasOwner", 0, portal.BusName).Store(&owned)
		if owned {
			break
		}
		if i > 50 {
			t.Fatal("the portal didn't start")
		}
		time.Sleep(100 * time.Millisecond)
	}
	chooser := conn.Object(portal.BusName, portal.Path)
	type reply struct {
		code uint32
		res  map[string]dbus.Variant
		err  error
	}
	call := func(handle dbus.ObjectPath, title string, opts map[string]dbus.Variant) chan reply {
		ch := make(chan reply, 1)
		go func() {
			var r reply
			r.err = chooser.Call(portal.Interface+".OpenFile", 0, handle, "org.example.App", "", title, opts).Store(&r.code, &r.res)
			ch <- r
		}()
		return ch
	}

	// Open: the app's filter (a GTK-style case-insensitive one) and folder.
	images := portal.Filter{Name: "Images", Patterns: []portalPattern{{0, "*.[pP][nN][gG]"}}}
	done := call("/org/freedesktop/portal/desktop/request/1/a", "Open Image", map[string]dbus.Variant{
		"filters":        dbus.MakeVariant([]portal.Filter{images}),
		"current_folder": dbus.MakeVariant([]byte(pics + "\x00")),
	})
	w := s.WaitWindow("Open Image — Files")
	w.Press("ctrl+2")
	w.Click(uitest.Named("Shot.PNG").Role("list item"))
	if _, ok := w.Lookup(uitest.Named("notes.txt").Role("list item")); ok {
		t.Error("notes.txt is offered for an image")
	}
	w.Click(uitest.Named("Open").Role("push button"))
	r := <-done
	if r.err != nil || r.code != portal.Success {
		t.Fatalf("open: %v %d", r.err, r.code)
	}
	if got := r.res["uris"].Value(); len(got.([]string)) != 1 || got.([]string)[0] != "file://"+filepath.Join(pics, "Shot.PNG") {
		t.Errorf("uris: %v", got)
	}

	// The app gives up (Request.Close): the picker goes, and it's cancelled.
	handle := dbus.ObjectPath("/org/freedesktop/portal/desktop/request/1/b")
	done = call(handle, "Open Something", map[string]dbus.Variant{})
	w = s.WaitWindow("Open Something — Files")
	if err := conn.Object(portal.BusName, handle).Call("org.freedesktop.impl.portal.Request.Close", 0).Err; err != nil {
		t.Fatal(err)
	}
	w.Closed()
	if r := <-done; r.err != nil || r.code != portal.Cancelled {
		t.Errorf("closed: %v %d", r.err, r.code)
	}
}
