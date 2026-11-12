//go:build uitest

// Click-and-check tests for the shell (bar, island, panels): run with
// `make ui-test`.
package uitest_test

import (
	"os"
	"path/filepath"
	"testing"

	"whiterose/daemon/internal/uitest"
)

// Quick settings: the tile turns Do Not Disturb on (stored, and the bell
// says so), and again off.
func TestQuickSettingsDoNotDisturb(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Shell: true})
	bar := s.Bar()
	bar.Click(uitest.Named("Quick settings").Role("push button"))
	dnd := uitest.Named("Do Not Disturb").Role("push button")
	if bar.WaitFor(dnd).Checked() {
		t.Fatal("Do Not Disturb starts on")
	}
	bar.Click(dnd)
	s.WaitSetting("quick.doNotDisturb", true)
	bar.WaitFor(uitest.Named("Notifications").Role("push button").Described("Do Not Disturb"))
	bar.WaitFor(dnd.Described("On"))
	bar.Click(dnd)
	s.WaitSetting("quick.doNotDisturb", false)
	bar.WaitFor(dnd.Described("Off"))
}

// Quick settings' "All settings" opens Settings.
func TestQuickSettingsOpensSettings(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Shell: true})
	bar := s.Bar()
	bar.Click(uitest.Named("Quick settings").Role("push button"))
	bar.Click(uitest.Named("All settings").Role("link"))
	s.WaitWindow("Settings")
	// (Opening another window closes the panel.)
	bar.Gone(uitest.Named("Do Not Disturb").Role("push button"))
}

// SUPER opens the launcher; a sum shows its answer; Esc closes it.
func TestLauncherCalculates(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Shell: true})
	bar := s.Bar()
	bar.Press("super")
	search := uitest.Named("Search").Role("text")
	bar.WaitFor(search)
	bar.Type("12*7")
	bar.WaitFor(uitest.Named("= 84").Role("list item").Described("12*7"))
	if got := bar.Text(search); got != "12*7" {
		t.Errorf("the search box holds %q", got)
	}
	bar.Press("Escape")
	bar.Gone(search)
}

// The bar's layout button shows this workspace's window layout; clicking
// it moves to the next one (as SUPER+T) and saves it for the workspace.
func TestBarLayoutButtonCyclesTheLayout(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Shell: true})
	bar := s.Bar()
	button := uitest.Named("Layout").Role("push button")
	bar.WaitFor(button.Described("Floating"))
	bar.Click(button)
	s.WaitSetting("windows.layout.workspaces", map[string]any{"1": "dwindle"})
	bar.WaitFor(button.Described("Dwindle"))
	bar.Click(button)
	s.WaitSetting("windows.layout.workspaces", map[string]any{"1": "master"})
	bar.WaitFor(button.Described("Master"))
	bar.Click(button)
	s.WaitSetting("windows.layout.workspaces", map[string]any{"1": "floating"})
	bar.WaitFor(button.Described("Floating"))
}

// SUPER+Z opens the snap layouts for the focused window (the left half
// chosen to start with); clicking the first third puts the window there.
func TestSnapPickerPutsTheWindowInACell(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Shell: true})
	dir := filepath.Join(s.Home, "snap")
	os.MkdirAll(dir, 0o755)
	files := s.Open(filesBinary(s)+" "+dir, "snap — Files")
	screen := s.Bar().Geometry()
	files.Press("super+z")
	picker := s.Layer("whiterose-snap-picker")
	picker.WaitFor(uitest.Named("Halves 1").Role("push button").Described("Chosen"))
	picker.Click(uitest.Named("Thirds 1").Role("push button"))
	var g [4]int
	inThird := uitest.WaitUntil(func() bool {
		g = files.Geometry()
		return g[0] < 20 && abs(3*g[2]-screen[2]) < 60
	})
	if !inThird {
		t.Fatalf("not in the left third of a %dpx screen: %v", screen[2], g)
	}
}

// Resting on a WhiteRose app's Maximise button shows the snap layouts
// just below it; the second half puts the window on the right.
func TestRestingOnMaximiseShowsTheSnapLayouts(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Shell: true})
	dir := filepath.Join(s.Home, "snap")
	os.MkdirAll(dir, 0o755)
	files := s.Open(filesBinary(s)+" "+dir, "snap — Files")
	screen := s.Bar().Geometry()
	files.Hover(uitest.Named("Maximise").Role("push button"))
	picker := s.Layer("whiterose-snap-picker")
	picker.Click(uitest.Named("Halves 2").Role("push button"))
	var g [4]int
	onTheRight := uitest.WaitUntil(func() bool {
		g = files.Geometry()
		return g[0] > screen[2]/2-20 && abs(2*g[2]-screen[2]) < 60
	})
	if !onTheRight {
		t.Fatalf("not in the right half of a %dpx screen: %v", screen[2], g)
	}
}

func abs(n int) int {
	if n < 0 {
		return -n
	}
	return n
}
