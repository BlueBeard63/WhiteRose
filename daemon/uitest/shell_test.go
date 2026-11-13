//go:build uitest

// Click-and-check tests for the shell (bar, island, panels): run with
// `make ui-test`.
package uitest_test

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	"whiterose/daemon/internal/uitest"
)

// Quick settings: the tile turns Do not disturb on (stored, and the bell
// says so), and again off.
func TestQuickSettingsDoNotDisturb(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Shell: true})
	bar := s.Bar()
	bar.Click(uitest.Named("Quick settings").Role("push button"))
	dnd := uitest.Named("Do not disturb").Role("push button")
	if bar.WaitFor(dnd).Checked() {
		t.Fatal("Do not disturb starts on")
	}
	bar.Click(dnd)
	s.WaitSetting("quick.doNotDisturb", true)
	bar.WaitFor(uitest.Named("Notifications").Role("push button").Described("Do not disturb"))
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
	bar.Gone(uitest.Named("Do not disturb").Role("push button"))
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
	files := s.Open(filesBinary(s)+" "+dir, "snap - Files")
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
	files := s.Open(filesBinary(s)+" "+dir, "snap - Files")
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

// PrtSc saves a screenshot and shows the toast; clicking its Delete button
// removes the file again (the toast's input region once stayed where its
// slide-in started, so clicks went through it).
func TestScreenshotToastDeletes(t *testing.T) {
	if uitest.Headless() {
		t.Skip("the headless backend draws nothing to capture")
	}
	s := uitest.Start(t, uitest.Options{Shell: true})
	s.Bar().Press("Print")
	toast := s.Layer("whiterose-toast")
	toast.WaitFor(uitest.Named("Open folder").Role("push button"))
	shots, _ := filepath.Glob(filepath.Join(s.Home, "Pictures/Screenshots/*.png"))
	if len(shots) != 1 {
		t.Fatalf("screenshots saved: %v", shots)
	}
	toast.Click(uitest.Named("Delete").Role("push button"))
	for i := 0; i < 50; i++ {
		if _, err := os.Stat(shots[0]); os.IsNotExist(err) {
			return
		}
		time.Sleep(100 * time.Millisecond)
	}
	t.Fatalf("%s is still there after Delete", shots[0])
}

// SUPER+V shows the clipboard history in the island; again, it goes.
func TestSuperVShowsTheClipboard(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Shell: true})
	bar := s.Bar()
	tab := uitest.Named("Clipboard").Role("page tab")
	bar.Press("super+v")
	if !bar.WaitFor(tab).Selected() {
		t.Error("the island isn't on the clipboard")
	}
	bar.Press("super+v")
	bar.Gone(tab)
}

// SUPER+SHIFT+N opens the note: written into, saved as it goes; with
// tasks (written "[]" or "- [ ]" too), Tab shows the checklist, clicking one
// ticks it, and Tab writes again.
func TestNoteWritesAndTicksTasks(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Shell: true})
	bar := s.Bar()
	bar.Press("super+shift+n")
	field := uitest.Named("Note").Role("text")
	bar.WaitFor(field)
	bar.Type("Today\n[] Call mom\n- [ ] Gym")
	s.WaitSetting("note.text", "Today\n[] Call mom\n- [ ] Gym")
	bar.Press("Tab")
	task := uitest.Named("Call mom").Role("check box")
	bar.WaitFor(task)
	bar.WaitFor(uitest.Named("0 of 2 done"))
	bar.Click(task)
	s.WaitSetting("note.text", "Today\n[x] Call mom\n- [ ] Gym")
	bar.WaitFor(uitest.Named("1 of 2 done"))
	bar.Press("Tab")
	bar.WaitFor(field)
}

// The Note module turned off (Settings › Modules): its key opens nothing.
func TestNoteModuleOff(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Shell: true, Settings: map[string]any{"settings.modules.disabled": []string{"note"}}})
	bar := s.Bar()
	bar.Press("super+shift+n")
	time.Sleep(time.Second)
	if _, ok := bar.Lookup(uitest.Named("Note").Role("text")); ok {
		t.Error("the note opened with its module off")
	}
}

// agenda seeds a calendar with an event on now (from ten minutes ago, for
// an hour), and returns what its subtitle says ("now · until 22:05").
func agenda(t *testing.T, more map[string]any) (*uitest.Session, string) {
	t.Helper()
	now := time.Now()
	if now.Hour() == 23 {
		t.Skip("the event would run past midnight")
	}
	start := now.Add(-10 * time.Minute).Truncate(time.Minute)
	end := start.Add(time.Hour)
	f := func(t time.Time) string { return t.Format("20060102T150405") }
	ics := "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:review@test\r\nDTSTART:" + f(start) +
		"\r\nDTEND:" + f(end) + "\r\nSUMMARY:Design review\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
	path := filepath.Join(t.TempDir(), "work.ics")
	os.WriteFile(path, []byte(ics), 0o644)
	settings := map[string]any{"calendar.sources": []map[string]string{{"name": "Work", "path": path}}}
	for k, v := range more {
		settings[k] = v
	}
	return uitest.Start(t, uitest.Options{Shell: true, Settings: settings}), "now · until " + end.Format("15:04") + " · Work"
}

// The clock opens today's agenda (the event on now, until when);
// "Month" opens the month; SUPER+A opens the agenda too.
func TestClockOpensTheAgenda(t *testing.T) {
	s, when := agenda(t, nil)
	bar := s.Bar()
	bar.Click(uitest.Named("Clock").Role("push button"))
	bar.WaitFor(uitest.Named("Design review"))
	bar.WaitFor(uitest.Named(when))
	bar.Click(uitest.Named("Month").Role("push button"))
	bar.WaitFor(uitest.Named("Next month").Role("push button"))
	bar.Press("Escape")
	bar.Gone(uitest.Named("Next month").Role("push button"))
	bar.Press("super+a")
	bar.WaitFor(uitest.Named("Design review"))
}

// The Agenda module off: the clock opens the month, as before.
func TestClockOpensTheMonthWithTheAgendaOff(t *testing.T) {
	s, _ := agenda(t, map[string]any{"settings.modules.disabled": []string{"agenda"}})
	bar := s.Bar()
	bar.Click(uitest.Named("Clock").Role("push button"))
	bar.WaitFor(uitest.Named("Next month").Role("push button"))
	if _, ok := bar.Lookup(uitest.Named("Design review")); ok {
		t.Error("the agenda opened with its module off")
	}
}

// "Now playing" opens what's playing in the island: the track, its
// controls (which reach the player), shuffle and repeat as the player has
// them. (A playing player is sent Pause, or PlayPause.)
func TestNowPlayingOpensTheMusicIsland(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Shell: true})
	p := fakePlayer(t, s)
	bar := s.Bar()
	bar.Click(uitest.Named("Now playing").Role("push button"))
	bar.WaitFor(uitest.Named("Midnight City").Role("label"))
	bar.Click(uitest.Named("Next").Role("push button"))
	bar.Click(uitest.Named("Pause").Role("push button"))
	if !uitest.WaitUntil(func() bool {
		c := p.Calls()
		return len(c) >= 2 && c[0] == "Next" && (c[1] == "Pause" || c[1] == "PlayPause")
	}) {
		t.Errorf("the player was sent %v", p.Calls())
	}
	bar.Click(uitest.Named("Shuffle").Role("push button"))
	if !uitest.WaitUntil(func() bool { v, _ := p.props.Get("org.mpris.MediaPlayer2.Player", "Shuffle"); return v.Value() == true }) {
		t.Error("shuffle didn't turn on")
	}
}

// With two players (Spotify paused, a browser playing a video), the island
// shows the one playing, and follows when they swap.
func TestTheMusicIslandFollowsThePlayerThatsPlaying(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Shell: true})
	spotify := namedPlayer(t, s, "spotify", "Paused Song", "Paused")
	browser := namedPlayer(t, s, "firefox", "A Video", "Playing")
	bar := s.Bar()
	bar.Click(uitest.Named("Now playing").Role("push button"))
	bar.WaitFor(uitest.Named("A Video").Role("label"))
	browser.props.SetMust("org.mpris.MediaPlayer2.Player", "PlaybackStatus", "Paused")
	spotify.props.SetMust("org.mpris.MediaPlayer2.Player", "PlaybackStatus", "Playing")
	bar.WaitFor(uitest.Named("Paused Song").Role("label"))
	// Paused, it stays on the last one that played.
	spotify.props.SetMust("org.mpris.MediaPlayer2.Player", "PlaybackStatus", "Paused")
	time.Sleep(300 * time.Millisecond)
	bar.WaitFor(uitest.Named("Paused Song").Role("label"))
}

// The Music module off: "Now playing" plays and pauses, as before.
func TestNowPlayingPausesWithTheMusicModuleOff(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Shell: true, Settings: map[string]any{"settings.modules.disabled": []string{"music"}}})
	p := fakePlayer(t, s)
	bar := s.Bar()
	bar.Click(uitest.Named("Now playing").Role("push button"))
	if !uitest.WaitUntil(func() bool { c := p.Calls(); return len(c) == 1 && (c[0] == "Pause" || c[0] == "PlayPause") }) {
		t.Errorf("the player was sent %v", p.Calls())
	}
	if _, ok := bar.Lookup(uitest.Named("Next").Role("push button")); ok {
		t.Error("the music island opened with its module off")
	}
}
