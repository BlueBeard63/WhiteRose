//go:build uitest

// Click-and-check tests for Settings: run with `make ui-test`.
package uitest_test

import (
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"testing"
	"time"

	"github.com/godbus/dbus/v5"

	"whiterose/daemon/internal/uitest"
)

// Turning on Seconds (Top bar) stores it and the bar's clock shows them.
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
	var problems []string
	seen := eachPage(t, settings, func(name string) {
		for _, n := range uitest.Unnamed(settings.LaidOut()) {
			problems = append(problems, fmt.Sprintf("%s: a %s with no name at %d,%d (%dx%d)", name, n.Role, n.X, n.Y, n.W, n.H))
		}
	})
	if len(seen) < 15 {
		t.Errorf("only %d pages: %v", len(seen), seen)
	}
	// A page that doesn't load has no unnamed controls either: the shell
	// says so in the log (a QML error).
	for _, line := range strings.Split(s.Log(), "\n") {
		if strings.Contains(line, "page failed to load") {
			problems = append(problems, strings.TrimSpace(line))
		}
	}
	for _, p := range problems {
		t.Error(p)
	}
}

// eachPage opens every page in Settings' sidebar in turn and calls `visit`
// on it; the pages it opened.
func eachPage(t *testing.T, settings *uitest.Window, visit func(name string)) map[string]bool {
	t.Helper()
	// The window can be in the tree before its items are (a slow machine).
	settings.WaitFor(uitest.Named("Apps").Role("page tab"))
	seen := map[string]bool{}
	for {
		var next *uitest.Node
		for _, n := range settings.LaidOut() {
			if n.Role == "page tab" && !seen[n.Name] {
				next = &n
				break
			}
		}
		if next == nil {
			return seen
		}
		seen[next.Name] = true
		tab := uitest.Named(next.Name).Role("page tab")
		settings.Activate(tab)
		if !uitest.WaitUntil(func() bool { n, _ := settings.Lookup(tab); return n.Selected() }) {
			t.Errorf("%s: its tab didn't open", next.Name)
			continue
		}
		visit(next.Name)
	}
}

// After every page has been open (and started what it watches), the
// Close button still quits Settings, not just hides it.
func TestSettingsQuitsAfterEveryPage(t *testing.T) {
	s := uitest.Start(t, uitest.Options{})
	settings := s.Settings("")
	if s.Running("settings.qml") == 0 {
		t.Fatal("can't find Settings' process")
	}
	eachPage(t, settings, func(string) {})
	settings.Click(uitest.Named("Close").Role("push button"))
	settings.Closed()
	settingsQuits(t, s)
}

// The window's own Close button closes Settings.
func TestSettingsCloses(t *testing.T) {
	s := uitest.Start(t, uitest.Options{})
	settings := s.Settings("")
	settings.Click(uitest.Named("Close").Role("push button"))
	settings.Closed()
	settingsQuits(t, s)
}

// settingsQuits fails the test if Settings' process outlives its window.
func settingsQuits(t *testing.T, s *uitest.Session) {
	t.Helper()
	if !uitest.WaitUntil(func() bool { return s.Running("settings.qml") == 0 }) {
		t.Error("Settings' window closed but its process is still running")
	}
}

// Closed from outside (SUPER+Q), Settings quits, so opening it again (from
// the launcher) brings a window back instead of finding a windowless one.
func TestSettingsOpensAgainAfterSuperQ(t *testing.T) {
	s := uitest.Start(t, uitest.Options{})
	settings := s.Settings("")
	settings.Press("super+q")
	settings.Closed()
	settingsQuits(t, s)
	s.Settings("")
}

// Opening Settings while it's open behind another window brings it forward.
func TestSettingsOpenedAgainComesForward(t *testing.T) {
	s := uitest.Start(t, uitest.Options{})
	s.Settings("")
	dir := t.TempDir()
	s.Open(filesBinary(s)+" "+dir, filepath.Base(dir)+" - Files")
	s.Spawn(filepath.Join(s.Repo, "shell/bin/whiterose-settings"))
	if !uitest.WaitUntil(func() bool { _, focused, _ := s.Toplevel("Settings"); return focused }) {
		t.Error("Settings didn't come forward")
	}
	// The second whiterose-settings handed off and exited: the compositor
	// collects it rather than leaving it a zombie.
	if !uitest.WaitUntil(func() bool { return s.Zombies() == 0 }) {
		t.Errorf("%d zombie processes left in the session", s.Zombies())
	}
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
	files := s.Open(filesBinary(s)+" "+dir, "tiles - Files")
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

// Settings › Snapping: with the Stacked tool, click a monitor's first zone
// (two thirds) to split it (saved as that monitor's grid), then
// SUPER+SHIFT-drag a window onto the lower half: it fills it. (A zone narrower than the window's
// smallest size, like the default grid's third, the app wouldn't take.)
func TestSettingsSplitZoneTakesASnappedWindow(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Shell: true})
	dir := filepath.Join(s.Home, "zones")
	os.MkdirAll(dir, 0o755)
	files := s.Open(filesBinary(s)+" "+dir, "zones - Files")
	settings := s.Settings("snapping")
	// (Maximised, so the end of the page is on the test's small screen.)
	settings.Click(uitest.Named("Maximise").Role("push button"))
	stacked := uitest.Named("Stacked").Role("radio button")
	settings.Click(stacked)
	if !uitest.WaitUntil(func() bool { n, ok := settings.Find(stacked); return ok && n.Checked() }) {
		t.Fatal("the Stacked tool didn't stay picked")
	}
	settings.Click(uitest.Named("Zone 1").Role("push button").Described("Split stacked"))
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

// Narrow (a half-screen tile): the sidebar is an icon rail; Menu slides the
// full sidebar over the page, and picking a page there opens it and slides
// the sidebar away. Every control on screen still has a name.
func TestSettingsNarrowRailOpensTheSidebarOverThePage(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Shell: true, Settings: map[string]any{"windows.layout": "master", "windows.master.ratio": 0.5}})
	settings := s.Settings("windows")
	settings.WaitFor(uitest.Named("Windows").Role("page tab"))
	dir := filepath.Join(s.Home, "beside")
	os.MkdirAll(dir, 0o755)
	s.Open(filesBinary(s)+" "+dir, "beside - Files")
	screen := s.Layer("whiterose-wallpaper").Geometry()
	if !uitest.WaitUntil(func() bool { return settings.Geometry()[2] < screen[2]*2/3 }) {
		t.Fatalf("Settings isn't a half tile: %v on %v", settings.Geometry(), screen)
	}
	searchBox := uitest.Named("Search settings").Role("text")
	menu := uitest.Named("Menu").Role("push button")
	settings.WaitFor(menu)
	// (Settings was wide until Files opened beside it: the sidebar slides
	// away into the rail, which takes a moment drawn in software.)
	if !uitest.WaitUntil(func() bool { n, ok := settings.Find(searchBox); return !ok || !n.Showing() }) {
		t.Fatal("the sidebar's search box shows before Menu opened it")
	}
	settings.Click(menu)
	settings.WaitFor(searchBox)
	settings.Activate(uitest.Named("Snapping").Role("page tab"))
	settings.WaitFor(uitest.Named("Stacked").Role("radio button"))
	if !uitest.WaitUntil(func() bool { n, ok := settings.Find(searchBox); return !ok || !n.Showing() }) {
		t.Error("picking a page didn't slide the sidebar away")
	}
	for _, n := range uitest.Unnamed(settings.LaidOut()) {
		t.Errorf("narrow: a %s with no name at %d,%d (%dx%d)", n.Role, n.X, n.Y, n.W, n.H)
	}
}

// Settings › Snapping: Delete takes the default grid's second zone away
// (empty space is allowed), and Draw drags a new zone over that space: its
// edges snap to the zone beside it and to the screen's edges, so it fills
// the right third again.
func TestSettingsDeleteAndDrawAZone(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Shell: true})
	settings := s.Settings("snapping")
	settings.Click(uitest.Named("Maximise").Role("push button"))
	settings.Click(uitest.Named("Delete").Role("radio button"))
	settings.Click(uitest.Named("Zone 2").Role("push button").Described("Delete"))
	if !uitest.WaitUntil(func() bool { return strings.Contains(s.Setting("windows.snap.zones"), "[[0,0,0.6667,1]]") }) {
		t.Fatalf("zone 2 wasn't deleted: %s", s.Setting("windows.snap.zones"))
	}
	settings.Click(uitest.Named("Draw").Role("radio button"))
	z1 := settings.WaitFor(uitest.Named("Zone 1").Role("push button"))
	g := settings.Geometry()
	// The editor's area: zone 1 is its left two thirds (less half a gap).
	area := float64(z1.W+2) / 0.6667
	left, top := float64(g[0]+z1.X), float64(g[1]+z1.Y)
	s.Drag(left+area*0.677, top+float64(z1.H)*0.01, left+area*0.99, top+float64(z1.H)*0.99)
	want := "[[0,0,0.6667,1],[0.6667,0,0.3333,1]]"
	if !uitest.WaitUntil(func() bool { return strings.Contains(s.Setting("windows.snap.zones"), want) }) {
		t.Fatalf("the drawn zone isn't the right third: %s", s.Setting("windows.snap.zones"))
	}
}

// Settings › Snapping: "Zones for" Workspace 1 edits that workspace's own
// grid (it starts as the monitor's): deleting a zone saves it there and
// leaves the monitor's grid alone; Reset takes the workspace back to it.
func TestSettingsWorkspaceHasItsOwnZones(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Shell: true})
	settings := s.Settings("snapping")
	settings.Click(uitest.Named("Maximise").Role("push button"))
	settings.Click(uitest.Named("Zones for").Role("combo box"))
	settings.WaitFor(uitest.Named("Workspace 1").Role("menu item"))
	settings.Activate(uitest.Named("Workspace 1").Role("menu item"))
	settings.Click(uitest.Named("Delete").Role("radio button"))
	settings.Click(uitest.Named("Zone 2").Role("push button").Described("Delete"))
	own := func() string { return s.Setting("windows.snap.zones.workspaces") }
	if !uitest.WaitUntil(func() bool { return strings.Contains(own(), `{"1":{`) && strings.Contains(own(), "[[0,0,0.6667,1]]") }) {
		t.Fatalf("not saved as workspace 1's own: %s", own())
	}
	if g := s.Setting("windows.snap.zones"); g != "" && g != "{}" {
		t.Errorf("the monitor's grid changed: %s", g)
	}
	settings.Click(uitest.Named("Reset").Role("push button"))
	s.WaitSetting("windows.snap.zones.workspaces", map[string]any{})
}

// Settings › Snapping: "Edit on screen" has the shell draw the zones over
// the monitor at full size, with the tools in its island; a zone deleted
// there is saved as in Settings, and Done ends it.
func TestSettingsEditZonesOnScreen(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Shell: true})
	settings := s.Settings("snapping")
	settings.Click(uitest.Named("Edit on screen").Role("push button"))
	overlay := s.Layer("whiterose-zone-editor")
	// The tools are in the island, over the zones.
	bar := s.Bar()
	del := uitest.Named("Delete").Role("radio button")
	bar.Click(del)
	if !uitest.WaitUntil(func() bool { n, ok := bar.Find(del); return ok && n.Checked() }) {
		t.Fatal("the Delete tool wasn't picked in the island")
	}
	overlay.Click(uitest.Named("Zone 2").Role("push button").Described("Delete"))
	if !uitest.WaitUntil(func() bool { return strings.Contains(s.Setting("windows.snap.zones"), "[[0,0,0.6667,1]]") }) {
		t.Fatalf("the zone deleted on screen wasn't saved: %s", s.Setting("windows.snap.zones"))
	}
	done := uitest.Named("Done").Role("push button")
	bar.Click(done)
	if !uitest.WaitUntil(func() bool { return !s.HasLayer("whiterose-zone-editor") }) {
		t.Error("Done didn't close the editor on screen")
	}
	bar.Gone(done)
}

// Settings › Window rules: a rule added from an open window matches its app
// id exactly and opens for editing; what's picked there (workspace, float,
// zone, opacity, on top) is saved, and deleting the rule saves none.
func TestSettingsAddARuleFromAnOpenWindow(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Shell: true})
	dir := filepath.Join(s.Home, "ruled")
	os.MkdirAll(dir, 0o755)
	s.Open(filesBinary(s)+" "+dir, "ruled - Files")
	settings := s.Settings("rules")
	settings.Click(uitest.Named("Maximise").Role("push button"))
	settings.WaitFor(uitest.Named("Add a blank rule").Role("push button"))
	var add *uitest.Node
	if !uitest.WaitUntil(func() bool {
		for _, n := range settings.LaidOut() {
			if n.Role == "push button" && strings.HasPrefix(n.Name, "Add a rule for ") && strings.Contains(n.Name, "files") {
				add = &n
				return true
			}
		}
		return false
	}) {
		t.Fatal("Files isn't offered as an open window")
	}
	app := strings.TrimPrefix(add.Name, "Add a rule for ")
	settings.Click(uitest.Named(add.Name).Role("push button"))
	want := `"app":"^` + strings.ReplaceAll(regexp.QuoteMeta(app), `\`, `\\`) + `$"`
	if !uitest.WaitUntil(func() bool { return strings.Contains(s.Setting("windows.rules"), want) }) {
		t.Fatalf("no rule for %s: %s", app, s.Setting("windows.rules"))
	}

	settings.Click(uitest.Named("Workspace").Role("combo box"))
	settings.WaitFor(uitest.Named("Workspace 3").Role("menu item"))
	settings.Activate(uitest.Named("Workspace 3").Role("menu item"))
	settings.Click(uitest.Named("Float").Role("radio button"))
	if !uitest.WaitUntil(func() bool {
		v := s.Setting("windows.rules")
		return strings.Contains(v, `"workspace":3`) && strings.Contains(v, `"float":true`)
	}) {
		t.Fatalf("the workspace and float weren't saved: %s", s.Setting("windows.rules"))
	}

	settings.Click(uitest.Named("Zone").Role("combo box"))
	settings.WaitFor(uitest.Named("Zone 2").Role("menu item"))
	settings.Activate(uitest.Named("Zone 2").Role("menu item"))
	if !uitest.WaitUntil(func() bool { return strings.Contains(s.Setting("windows.rules"), `"zone":2`) }) {
		t.Fatalf("the zone wasn't saved: %s", s.Setting("windows.rules"))
	}
	settings.Click(uitest.Named("Opacity").Role("check box"))
	settings.ScrollTo(uitest.Named("Always on top").Role("check box"))
	settings.Click(uitest.Named("Always on top").Role("check box"))
	settings.WaitFor(uitest.Named("When not focused").Role("slider"))
	if !uitest.WaitUntil(func() bool {
		v := s.Setting("windows.rules")
		return strings.Contains(v, `"opacity":[1,0.85]`) && strings.Contains(v, `"onTop":true`)
	}) {
		t.Fatalf("the opacity and on top weren't saved: %s", s.Setting("windows.rules"))
	}

	// (Scrolled back up just far enough, its header can sit under the
	// window's top strip: press it by its action, not a click.)
	settings.Activate(uitest.Named("Delete rule 1").Role("push button"))
	s.WaitSetting("windows.rules", []any{})
}

// Settings › Scratchpads: a new pad starts as the terminal with SUPER+`;
// given another command and placed in the middle, it's saved. Its key then
// starts that app, whose window is the pad's (off the window list), and
// shows and hides it.
func TestSettingsScratchpadIsAddedAndItsKeyStartsIt(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Shell: true, Settings: map[string]any{"apps.terminal": "ghostty"}})
	settings := s.Settings("scratchpads")
	settings.Click(uitest.Named("Add a scratchpad").Role("push button"))
	if !uitest.WaitUntil(func() bool {
		v := s.Setting("windows.scratchpads")
		return strings.Contains(v, `"command":"ghostty --gtk-single-instance=false"`) && strings.Contains(v, `"keys":["SUPER+GRAVE"]`)
	}) {
		t.Fatalf("no terminal scratchpad: %s", s.Setting("windows.scratchpads"))
	}

	dir := filepath.Join(s.Home, "padded")
	os.MkdirAll(dir, 0o755)
	settings.Click(uitest.Named("Command").Role("text"))
	settings.Press("ctrl+a")
	settings.Type(filesBinary(s) + " " + dir + "\n")
	settings.Click(uitest.Named("Centred").Role("radio button"))
	if !uitest.WaitUntil(func() bool {
		v := s.Setting("windows.scratchpads")
		return strings.Contains(v, `whiterose-files `+dir) && strings.Contains(v, `"place":"centre"`)
	}) {
		t.Fatalf("the command and place weren't saved: %s", s.Setting("windows.scratchpads"))
	}

	// Its key on the Keyboard page, under Scratchpads.
	keyboard := s.Settings("keyboard")
	keyboard.WaitFor(uitest.Named("Scratchpads").Role("radio button"))

	settings.Press("super+`")
	if !uitest.WaitUntil(func() bool { return strings.Contains(s.Log(), "scratchpad window opened") }) {
		t.Fatalf("the key didn't start a scratchpad:\n%s", s.Log())
	}
	if s.HasWindow("padded - Files") {
		t.Fatal("the scratchpad's window is in the window list")
	}
	// Hidden and shown again: still the one window, nothing new started.
	settings.Press("super+`")
	settings.Press("super+`")
	time.Sleep(300 * time.Millisecond)
	if n := strings.Count(s.Log(), "scratchpad window opened"); n != 1 {
		t.Fatalf("%d scratchpad windows opened, want 1", n)
	}
}

// Settings › Files › Search › Make again: whiterosed makes the index with
// Files never opened, and the row says when.
func TestSettingsMakesTheSearchIndexWithoutFiles(t *testing.T) {
	s := uitest.Start(t, uitest.Options{})
	os.WriteFile(filepath.Join(s.Home, "Documents", "plan.txt"), nil, 0o644)
	settings := s.Settings("files/search")
	settings.Click(uitest.Named("Make again").Role("push button"))
	index := filepath.Join(s.Home, ".cache/whiterose/files/index")
	deadline := time.Now().Add(20 * time.Second)
	for {
		if _, err := os.Stat(index); err == nil {
			break
		}
		if time.Now().After(deadline) {
			t.Fatal("no index after Make again")
		}
		time.Sleep(100 * time.Millisecond)
	}
	if s.HasWindow("Home - Files") || s.HasWindow("Files") {
		t.Error("a Files window opened")
	}
	// Done: the button's back (it's off while the index is being made).
	for !settings.WaitFor(uitest.Named("Make again").Role("push button")).Enabled() {
		if time.Now().After(deadline) {
			t.Fatal("Make again stays off after the index was made")
		}
		time.Sleep(200 * time.Millisecond)
	}
}

// Mouse & touchpad: the mouse scrolls naturally once picked, and tap to
// click and shake to find turn off; all stored for the compositor.
func TestSettingsMouseAndTouchpad(t *testing.T) {
	s := uitest.Start(t, uitest.Options{})
	settings := s.Settings("mouse")
	// (The mouse's row comes first; the touchpad's is natural already.)
	settings.Click(uitest.Named("Natural").Role("radio button"))
	s.WaitSetting("mouse.naturalScroll", true)
	settings.Click(uitest.Named("Tap to click").Role("check box"))
	s.WaitSetting("touchpad.tapToClick", false)
	settings.Click(uitest.Named("Shake to find").Role("check box"))
	s.WaitSetting("mouse.shakeToFind", false)
	if got := s.Setting("touchpad.naturalScroll"); got != "true" {
		t.Errorf("touchpad.naturalScroll = %s", got)
	}
}

// Mouse & touchpad › Faster while you hold: Ctrl+Shift to start with;
// another pick is stored, and how much faster follows a click and then
// the wheel.
func TestSettingsFasterPointer(t *testing.T) {
	s := uitest.Start(t, uitest.Options{})
	settings := s.Settings("mouse")
	settings.Click(uitest.Named("Faster while you hold").Role("combo box").Described("Ctrl + Shift"))
	settings.Activate(uitest.Named("Super + Shift"))
	s.WaitSetting("mouse.fasterWith", "SUPER+SHIFT")
	// (Clicked first: the wheel only moves a slider once it's clicked.
	// The middle is 3.5×; two turns up, 4.5×.)
	settings.Click(uitest.Named("How much faster").Role("slider"))
	s.WaitSetting("mouse.fasterBy", 3.5)
	settings.Wheel(uitest.Named("How much faster").Role("slider"), -2)
	s.WaitSetting("mouse.fasterBy", 4.5)
}

// Settings chooses files and folders with Files' picker: a folder left out
// of the search index, and a wallpaper image.
func TestSettingsPicksWithFiles(t *testing.T) {
	s := uitest.Start(t, uitest.Options{})
	pics := filepath.Join(s.Home, "Pictures")
	os.WriteFile(filepath.Join(pics, "dunes.png"), []byte("x"), 0o644)

	settings := s.Settings("files/search")
	settings.Click(uitest.Named("Leave a folder out…").Role("push button"))
	picker := s.WaitWindow("A folder to leave out of the index - Files")
	picker.Press("ctrl+2")
	picker.Click(uitest.Named("Documents").Role("list item"))
	picker.Click(uitest.Named("Choose folder").Role("push button"))
	s.WaitSetting("files.indexExclude", []any{"~/Documents"})

	settings = s.Settings("appearance")
	settings.Click(uitest.Named("Add a wallpaper").Role("push button"))
	picker = s.WaitWindow("Choose a wallpaper - Files")
	picker.Press("ctrl+2")
	picker.Click(uitest.Named("dunes.png").Role("list item"))
	picker.Click(uitest.Named("Open").Role("push button"))
	s.WaitSetting("wallpaper.files", []any{"~/Pictures/dunes.png"})
}

// Windows › Switching: ALT+TAB can list only this workspace's apps. (At
// the end of the page, scrolled to past the gap and master sliders: they
// leave the wheel to the page.)
func TestSettingsAltTabThisWorkspace(t *testing.T) {
	s := uitest.Start(t, uitest.Options{})
	settings := s.Settings("windows")
	settings.Click(uitest.Named("This workspace").Role("radio button"))
	s.WaitSetting("windows.appSwitcher.workspaces", "current")
}

// A slider ignores the wheel until it's clicked: scrolling past one scrolls
// the page and leaves its value be; clicked, the wheel moves it.
func TestSettingsSlidersTakeTheWheelOnlyOnceClicked(t *testing.T) {
	s := uitest.Start(t, uitest.Options{})
	settings := s.Settings("keyboard")
	delay := uitest.Named("Repeat delay").Role("slider")
	before := settings.WaitFor(delay)
	settings.Wheel(delay, 3)
	time.Sleep(400 * time.Millisecond)
	if got := s.Setting("keyboard.repeatDelay"); got != "300" {
		t.Errorf("the wheel moved an unclicked slider: %s", got)
	}
	if after, _ := settings.Lookup(delay); after.Y >= before.Y {
		t.Errorf("the page didn't scroll (%d, then %d)", before.Y, after.Y)
	}
	settings.Click(delay)
	clicked := s.Setting("keyboard.repeatDelay")
	settings.Wheel(delay, -3)
	if !uitest.WaitUntil(func() bool { return s.Setting("keyboard.repeatDelay") != clicked }) {
		t.Errorf("clicked, the wheel didn't move it (still %s)", clicked)
	}
}

// A floating window gets smaller when its bottom-right corner is dragged in:
// Settings (a shell window), and Files and Notepad (the apps' AppWindow),
// each on its own.
func TestFloatingWindowsResizeFromTheirCorner(t *testing.T) {
	t.Run("Settings", func(t *testing.T) {
		s := uitest.Start(t, uitest.Options{})
		dragCornerIn(t, s, s.Settings(""))
	})
	t.Run("Files", func(t *testing.T) {
		s := uitest.Start(t, uitest.Options{})
		dragCornerIn(t, s, s.Open(filesBinary(s)+" "+s.Home, "Home - Files"))
	})
	t.Run("Notepad", func(t *testing.T) {
		s := uitest.Start(t, uitest.Options{})
		dragCornerIn(t, s, s.Open(filepath.Join(s.Repo, "apps/target/debug/whiterose-notepad"), "Scratch - Notepad"))
	})
}

func dragCornerIn(t *testing.T, s *uitest.Session, w *uitest.Window) {
	t.Helper()
	g := w.Geometry()
	// 4px in from the corner: on the corner grip, inside the window.
	x, y := float64(g[0]+g[2]-4), float64(g[1]+g[3]-4)
	s.Drag(x, y, x-120, y-80)
	var now [4]int
	if !uitest.WaitUntil(func() bool { now = w.Geometry(); return now[2] < g[2]-60 && now[3] < g[3]-40 }) {
		t.Errorf("dragging the corner didn't resize it: %v, then %v", g, now)
	}
}

// Password fields have their names (screen readers, tests) and show only
// dots. Opens Users › Change password and cancels: never saves, as it's
// the real account.
func TestSettingsPasswordFieldsAreNamed(t *testing.T) {
	needsAccounts(t)
	s := uitest.Start(t, uitest.Options{})
	settings := s.Settings("users")
	settings.Click(uitest.Named("Change password").Role("push button"))
	first := uitest.Named("New password").Role("text")
	settings.WaitFor(first)
	settings.WaitFor(uitest.Named("Again").Role("text"))
	settings.Type("hunter2")
	uitest.WaitUntil(func() bool { return settings.Text(first) != "" })
	if got := settings.Text(first); got == "" || strings.Contains(got, "hunter2") {
		t.Fatalf("the field reads %q", got)
	}
	settings.Click(uitest.Named("Cancel").Role("push button"))
	settings.Gone(first)
}

// Users lists accounts through AccountsService on the system bus; without
// it (CI's container) there's no account card to click.
func needsAccounts(t *testing.T) {
	t.Helper()
	bus, err := dbus.ConnectSystemBus()
	if err != nil {
		t.Skipf("no system bus: %v", err)
	}
	defer bus.Close()
	if err := bus.Object("org.freedesktop.Accounts", "/org/freedesktop/Accounts").Call("org.freedesktop.DBus.Peer.Ping", 0).Err; err != nil {
		t.Skipf("AccountsService isn't running: %v", err)
	}
}
