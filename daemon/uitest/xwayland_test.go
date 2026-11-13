//go:build uitest

// X11 apps through XWayland: they open, take the focus, draw, and close.
package uitest_test

import (
	"image"
	"os/exec"
	"testing"

	"whiterose/daemon/internal/uitest"
)

func needsX11(t *testing.T, programs ...string) {
	t.Helper()
	for _, p := range append([]string{"Xwayland"}, programs...) {
		if _, err := exec.LookPath(p); err != nil {
			t.Skipf("%s isn't installed", p)
		}
	}
}

// xmessage (plain X11) opens as a window with its WM_CLASS as the app id,
// focused; SUPER+Q closes it (the X11 way), and it goes.
func TestAnX11AppOpensFocusedAndCloses(t *testing.T) {
	needsX11(t, "xmessage")
	s := uitest.Start(t, uitest.Options{})
	s.Spawn("xmessage Hello from X11")
	var app string
	var focused bool
	if !uitest.WaitUntil(func() bool { var ok bool; app, focused, ok = s.Toplevel("xmessage"); return ok && focused }) {
		t.Fatalf("no focused xmessage window (app %q, focused %v)\n%s", app, focused, s.Log())
	}
	if app != "Xmessage" {
		t.Errorf("app id %q, not its WM_CLASS", app)
	}
	// It draws: its text and button, not a flat rectangle.
	var g [4]int
	if !uitest.WaitUntil(func() bool { var ok bool; g, ok = s.WindowGeometry("xmessage"); return ok && g[2] > 0 && g[3] > 0 }) {
		t.Fatalf("no size for xmessage")
	}
	uitest.WaitUntil(func() bool { return colours(s.Capture(g[0], g[1], g[2], g[3])) > 1 })
	if n := colours(s.Capture(g[0], g[1], g[2], g[3])); n < 2 {
		t.Errorf("xmessage's window is one colour (%d): it didn't draw", n)
	}
	s.Press("super+q")
	if !uitest.WaitUntil(func() bool { _, _, ok := s.Toplevel("xmessage"); return !ok }) {
		t.Error("SUPER+Q didn't close it")
	}
}

// colours counts the distinct colours in img (up to a few).
func colours(img image.Image) int {
	seen := map[[4]uint32]bool{}
	b := img.Bounds()
	for y := b.Min.Y; y < b.Max.Y; y += 2 {
		for x := b.Min.X; x < b.Max.X; x += 2 {
			r, g, bl, a := img.At(x, y).RGBA()
			seen[[4]uint32{r, g, bl, a}] = true
			if len(seen) > 8 {
				return len(seen)
			}
		}
	}
	return len(seen)
}
