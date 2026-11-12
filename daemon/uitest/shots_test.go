//go:build uitest && shots

// Screenshots for reviewing how things look, not checks: the bar's icons
// and Quick Settings, and every Settings page at the sizes it gets as a tile
// (a third, a half, two thirds, and half height). Run with `make shots`;
// they land in build/shots, with a contact sheet per Settings size.
package uitest_test

import (
	"fmt"
	"image"
	"image/color"
	"image/draw"
	"image/png"
	"os"
	"path/filepath"
	"regexp"
	"testing"
	"time"

	"whiterose/daemon/internal/uitest"
)

func TestShots(t *testing.T) {
	out := os.Getenv("WHITEROSE_SHOTS")
	if out == "" {
		t.Skip("WHITEROSE_SHOTS isn't set (make shots)")
	}
	// (Scale 1 only: a scale seeded in "displays" doesn't reach the
	// headless output yet.)
	t.Run("bar", func(t *testing.T) { shootBar(t, out, 1) })
	t.Run("settings", func(t *testing.T) { shootSettings(t, out) })
}

// The bar's left end (workspaces, the layout button in each layout) and
// right end (status icons, bell, clock), and Quick Settings open.
func shootBar(t *testing.T, out string, scale int) {
	opts := uitest.Options{Shell: true}
	if scale != 1 {
		opts.Settings = map[string]any{"displays": map[string]any{
			"monitor": []any{map[string]any{"id": "connector:HEADLESS-1", "scale": scale}},
			"profile": []any{},
		}}
	}
	s := uitest.Start(t, opts)
	bar := s.Bar()
	button := uitest.Named("Layout").Role("push button")
	bar.WaitFor(button.Described("Floating"))
	time.Sleep(500 * time.Millisecond)
	g := bar.Geometry()
	h := 26
	at := fmt.Sprintf("@%dx", scale)
	save(t, filepath.Join(out, "bar", "right"+at+".png"), s.Capture(g[2]-300, 0, 300, h))
	for _, layout := range []string{"Floating", "Dwindle", "Master", "Scrolling"} {
		bar.WaitFor(button.Described(layout))
		time.Sleep(200 * time.Millisecond)
		save(t, filepath.Join(out, "bar", "left-"+layout+at+".png"), s.Capture(0, 0, 300, h))
		bar.Click(button)
	}
	bar.Click(uitest.Named("Quick settings").Role("push button"))
	time.Sleep(800 * time.Millisecond)
	screen := s.Layer("whiterose-wallpaper").Geometry()
	save(t, filepath.Join(out, "bar", "quick-settings"+at+".png"), s.Capture(screen[2]/2, 0, screen[2]/2, screen[3]))
}

// Settings as a column of a scrolling workspace: half wide on its own;
// SUPER+W makes it two thirds, then a third; SUPER+[ pulls a Files window
// in below it (half height), and SUPER+W again makes that a half.
func shootSettings(t *testing.T, out string) {
	s := uitest.Start(t, uitest.Options{Shell: true, Settings: map[string]any{"windows.layout": "scrolling"}})
	settings := s.Settings("")
	settings.WaitFor(uitest.Named("Apps").Role("page tab"))
	screen := s.Layer("whiterose-wallpaper").Geometry()
	wide := func(frac float64) func([4]int) bool {
		return func(g [4]int) bool { return abs(g[2]-int(float64(screen[2])*frac)) < 40 }
	}
	sized := func(name string, ok func([4]int) bool) {
		if !uitest.WaitUntil(func() bool { return ok(settings.Geometry()) }) {
			t.Fatalf("%s: Settings is %v on a %dx%d screen", name, settings.Geometry(), screen[2], screen[3])
		}
		shootPages(t, s, settings, out, name)
	}
	sized("half", wide(0.5))
	settings.Press("super+w")
	sized("two-thirds", wide(2.0/3))
	settings.Press("super+w")
	sized("third", wide(1.0/3))

	dir := filepath.Join(s.Home, "short")
	os.MkdirAll(dir, 0o755)
	s.Open(filesBinary(s)+" "+dir, "short — Files")
	settings.Press("super+left")
	settings.Press("super+[")
	short := func(frac float64) func([4]int) bool {
		return func(g [4]int) bool { return wide(frac)(g) && g[3] < screen[3]/2 }
	}
	sized("third-short", short(1.0/3))
	settings.Press("super+w")
	sized("half-short", short(0.5))
}

var unsafeName = regexp.MustCompile(`[^A-Za-z0-9]+`)

// Every page of Settings at its current size, and a contact sheet of them.
func shootPages(t *testing.T, s *uitest.Session, settings *uitest.Window, out, size string) {
	var tabs []string
	for _, n := range settings.Nodes() {
		if n.Role == "page tab" {
			tabs = append(tabs, n.Name)
		}
	}
	var shots []image.Image
	for i, name := range tabs {
		tab := uitest.Named(name).Role("page tab")
		settings.Activate(tab)
		uitest.WaitUntil(func() bool { n, _ := settings.Lookup(tab); return n.Selected() })
		time.Sleep(400 * time.Millisecond)
		g := settings.Geometry()
		img := s.Capture(g[0], g[1], g[2], g[3])
		save(t, filepath.Join(out, "settings", size, fmt.Sprintf("%02d-%s.png", i+1, unsafeName.ReplaceAllString(name, "-"))), img)
		shots = append(shots, img)
	}
	save(t, filepath.Join(out, "settings-"+size+".png"), sheet(shots, 4))
}

// The pictures at half size in rows of `cols`, on a grey ground.
func sheet(imgs []image.Image, cols int) image.Image {
	if len(imgs) == 0 {
		return image.NewRGBA(image.Rect(0, 0, 1, 1))
	}
	cw, ch := 0, 0
	for _, m := range imgs {
		cw, ch = max(cw, m.Bounds().Dx()/2), max(ch, m.Bounds().Dy()/2)
	}
	const pad = 12
	rows := (len(imgs) + cols - 1) / cols
	out := image.NewRGBA(image.Rect(0, 0, cols*(cw+pad)+pad, rows*(ch+pad)+pad))
	draw.Draw(out, out.Bounds(), &image.Uniform{color.RGBA{0x80, 0x80, 0x88, 0xff}}, image.Point{}, draw.Src)
	for i, m := range imgs {
		x, y := pad+(i%cols)*(cw+pad), pad+(i/cols)*(ch+pad)
		half(out, m, x, y)
	}
	return out
}

// half draws `m` at half size (each 2x2 block averaged) at x, y in `dst`.
func half(dst *image.RGBA, m image.Image, x, y int) {
	b := m.Bounds()
	for j := 0; j < b.Dy()/2; j++ {
		for i := 0; i < b.Dx()/2; i++ {
			var r, g, bl, a uint32
			for _, d := range [4][2]int{{0, 0}, {1, 0}, {0, 1}, {1, 1}} {
				cr, cg, cb, ca := m.At(b.Min.X+2*i+d[0], b.Min.Y+2*j+d[1]).RGBA()
				r, g, bl, a = r+cr, g+cg, bl+cb, a+ca
			}
			dst.Set(x+i, y+j, color.RGBA64{uint16(r / 4), uint16(g / 4), uint16(bl / 4), uint16(a / 4)})
		}
	}
}

func save(t *testing.T, path string, img image.Image) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	f, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	if err := png.Encode(f, img); err != nil {
		t.Fatal(err)
	}
}
