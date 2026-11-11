//go:build uitest

// Click-and-check tests for the shell (bar, island, panels): run with
// `make ui-test`.
package uitest_test

import (
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
