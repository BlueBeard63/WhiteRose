//go:build uitest

// Click-and-check tests for Settings: run with `make ui-test`.
package uitest_test

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"whiterose/daemon/internal/uitest"
)

// Turning on Seconds (Top Bar) stores it and the bar's clock shows them.
func TestSettingsSecondsShowInTheBarClock(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Shell: true})
	bar := s.Bar()
	bar.WaitFor(uitest.Named("Clock").Role("push button").DescribedLike(`\d:\d\d$`))

	settings := s.Settings("topbar")
	seconds := settings.ScrollTo(uitest.Named("Seconds").Role("check box"))
	if seconds.Checked() {
		t.Fatal("Seconds starts on")
	}
	settings.Click(uitest.Named("Seconds").Role("check box"))
	s.WaitSetting("bar.seconds", true)
	bar.WaitFor(uitest.Named("Clock").Role("push button").DescribedLike(`\d:\d\d:\d\d$`))
	if n, _ := settings.Lookup(uitest.Named("Seconds").Role("check box")); !n.Checked() {
		t.Error("the Seconds switch didn't turn on")
	}
}

// Every page (and sub-page) of Settings: each control has a name, so a
// screen reader can say what it is and a test can find it.
func TestSettingsEveryControlHasAName(t *testing.T) {
	s := uitest.Start(t, uitest.Options{})
	settings := s.Settings("")
	// The window can be in the tree before its items are (a slow machine).
	settings.WaitFor(uitest.Named("Apps").Role("page tab"))
	seen := map[string]bool{}
	var problems []string
	for {
		var next *uitest.Node
		for _, n := range settings.LaidOut() {
			if n.Role == "page tab" && !seen[n.Name] {
				next = &n
				break
			}
		}
		if next == nil {
			break
		}
		seen[next.Name] = true
		tab := uitest.Named(next.Name).Role("page tab")
		settings.Activate(tab)
		if !uitest.WaitUntil(func() bool { n, _ := settings.Lookup(tab); return n.Selected() }) {
			problems = append(problems, fmt.Sprintf("%s: its tab didn't open", next.Name))
			continue
		}
		for _, n := range uitest.Unnamed(settings.LaidOut()) {
			problems = append(problems, fmt.Sprintf("%s: a %s with no name at %d,%d (%dx%d)", next.Name, n.Role, n.X, n.Y, n.W, n.H))
		}
	}
	if len(seen) < 15 {
		t.Errorf("only %d pages: %v", len(seen), seen)
	}
	for _, p := range problems {
		t.Error(p)
	}
}

// The window's own Close button closes Settings.
func TestSettingsCloses(t *testing.T) {
	s := uitest.Start(t, uitest.Options{})
	settings := s.Settings("")
	settings.Click(uitest.Named("Close").Role("push button"))
	settings.Closed()
}

// On a new install Settings opens by itself on Apps (the app setup).
func TestFirstRunOffersTheAppSetup(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Shell: true, FirstRun: true})
	settings := s.WaitWindow("Settings")
	if !settings.WaitFor(uitest.Named("Apps").Role("page tab")).Selected() {
		t.Error("Settings didn't open on Apps")
	}
}

// Windows: choosing a layout for workspace 1 stores it and tiles that
// workspace's windows (Settings and Files share the screen, side by side);
// back on Default, the setting is cleared.
func TestSettingsWorkspaceLayoutTilesItsWindows(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Shell: true})
	dir := filepath.Join(s.Home, "tiles")
	os.MkdirAll(dir, 0o755)
	files := s.Open(filesBinary(s)+" "+dir, "tiles — Files")
	settings := s.Settings("windows")
	// (A dropdown's items report places inside the popup, not the window,
	// so they're picked through their accessible action, not clicked.)
	pick := func(combo, item string) {
		settings.Click(uitest.Named(combo).Role("combo box"))
		settings.WaitFor(uitest.Named(item).Role("menu item"))
		settings.Activate(uitest.Named(item).Role("menu item"))
	}
	pick("Workspace 1", "Dwindle")
	s.WaitSetting("windows.layout.workspaces", map[string]any{"1": "dwindle"})
	var a, b [4]int
	tiled := uitest.WaitUntil(func() bool {
		a, b = files.Geometry(), settings.Geometry()
		apart := a[0]+a[2] <= b[0] || b[0]+b[2] <= a[0]
		return apart && a[1] == b[1] && a[3] == b[3]
	})
	if !tiled {
		t.Fatalf("not tiled side by side: Files %v, Settings %v", a, b)
	}
	pick("Workspace 1", "Default")
	s.WaitSetting("windows.layout.workspaces", map[string]any{})
}

// Windows: Scrolling for workspace 1 makes a strip of half-wide columns.
// Settings and three Files windows are four columns: the newest is in view
// and the first has scrolled off the left. SUPER+W makes the focused column
// two thirds wide, and the strip keeps it in view.
func TestSettingsScrollingKeepsTheNewestColumnInView(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Shell: true})
	settings := s.Settings("windows")
	settings.Click(uitest.Named("Workspace 1").Role("combo box"))
	settings.WaitFor(uitest.Named("Scrolling").Role("menu item"))
	settings.Activate(uitest.Named("Scrolling").Role("menu item"))
	s.WaitSetting("windows.layout.workspaces", map[string]any{"1": "scrolling"})
	var wins []*uitest.Window
	for _, name := range []string{"one", "two", "three"} {
		dir := filepath.Join(s.Home, name)
		os.MkdirAll(dir, 0o755)
		wins = append(wins, s.Open(filesBinary(s)+" "+dir, name+" — Files"))
	}
	screen := s.Bar().Geometry()
	first, newest := wins[0], wins[2]
	var a, c [4]int
	strip := uitest.WaitUntil(func() bool {
		a, c = first.Geometry(), newest.Geometry()
		return a[0]+a[2] <= 0 && c[0] >= 0 && c[0]+c[2] <= screen[2] && abs(2*c[2]-screen[2]) < 60
	})
	if !strip {
		t.Fatalf("on a %dpx screen, the first column %v isn't off the left or the newest %v isn't a half in view", screen[2], a, c)
	}
	newest.Press("super+w")
	wider := uitest.WaitUntil(func() bool {
		c = newest.Geometry()
		return abs(3*c[2]-2*screen[2]) < 60 && c[0] >= 0 && c[0]+c[2] <= screen[2]
	})
	if !wider {
		t.Fatalf("SUPER+W: not two thirds of a %dpx screen and in view: %v", screen[2], c)
	}
}

// Settings › Snapping: choose a monitor's first zone (two thirds) and split it
// stacked (saved as that monitor's grid), then SUPER+SHIFT-drag a window
// onto the lower half: it fills it. (A zone narrower than the window's
// smallest size, like the default grid's third, the app wouldn't take.)
func TestSettingsSplitZoneTakesASnappedWindow(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Shell: true})
	dir := filepath.Join(s.Home, "zones")
	os.MkdirAll(dir, 0o755)
	files := s.Open(filesBinary(s)+" "+dir, "zones — Files")
	settings := s.Settings("snapping")
	// (Maximised, so the end of the page is on the test's small screen.)
	settings.Click(uitest.Named("Maximise").Role("push button"))
	zone := uitest.Named("Zone 1").Role("check box")
	settings.Click(zone)
	if !uitest.WaitUntil(func() bool { n, ok := settings.Find(zone); return ok && n.Checked() }) {
		t.Fatal("clicking zone 1 didn't choose it")
	}
	settings.Click(uitest.Named("Split stacked").Role("push button"))
	if !uitest.WaitUntil(func() bool { return strings.Contains(s.Setting("windows.snap.zones"), "[0,0.5,0.6667,0.5]") }) {
		t.Fatalf("no split grid saved: %s", s.Setting("windows.snap.zones"))
	}
	settings.Click(uitest.Named("Close").Role("push button"))
	settings.Closed()

	screen := s.Layer("whiterose-wallpaper").Geometry()
	g := files.Geometry()
	bar := 26
	below := screen[3] - bar
	s.Drag(float64(g[0]+g[2]/2), float64(g[1]+g[3]/2), float64(screen[2])*0.3, float64(bar)+float64(below)*0.8, "super", "shift")
	inZone := uitest.WaitUntil(func() bool {
		g = files.Geometry()
		return g[0] < 20 && abs(g[1]-(bar+below/2)) < 20 && abs(3*g[2]-2*screen[2]) < 60
	})
	if !inZone {
		t.Fatalf("not in the lower left zone of a %dx%d screen: %v", screen[2], screen[3], g)
	}
}
