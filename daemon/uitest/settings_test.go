//go:build uitest

// Click-and-check tests for Settings: run with `make ui-test`.
package uitest_test

import (
	"fmt"
	"os"
	"path/filepath"
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
