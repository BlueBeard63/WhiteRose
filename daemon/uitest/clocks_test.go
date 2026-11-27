//go:build uitest

package uitest_test

import (
	"encoding/json"
	"image/png"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"

	"whiterose/daemon/internal/clocks"
	"whiterose/daemon/internal/uitest"
)

func clocksBinary(s *uitest.Session) string {
	if path := os.Getenv("WR_UITEST_CLOCKS"); path != "" {
		return path
	}
	return filepath.Join(s.Repo, "apps/target/debug/whiterose-clocks")
}
func clocksState(t *testing.T, s *uitest.Session) clocks.State {
	t.Helper()
	var state clocks.State
	if err := json.Unmarshal(s.Request("clocks_state", nil), &state); err != nil {
		t.Fatal(err)
	}
	return state
}
func clocksField(w *uitest.Window, name, value string) {
	w.Click(uitest.Named(name).Role("text"))
	w.Press("ctrl+a")
	w.Type(value)
}
func clocksShot(t *testing.T, s *uitest.Session, w *uitest.Window, name string) {
	t.Helper()
	out := os.Getenv("WHITEROSE_CLOCKS_SHOTS")
	if out == "" {
		return
	}
	// AT-SPI can observe updated text before Qt has painted the next frame.
	time.Sleep(200 * time.Millisecond)
	os.MkdirAll(out, 0o755)
	g := w.Geometry()
	img := s.Capture(g[0], g[1], g[2], g[3])
	f, err := os.Create(filepath.Join(out, name+".png"))
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	if err := png.Encode(f, img); err != nil {
		t.Fatal(err)
	}
}

func clocksShellShot(t *testing.T, s *uitest.Session, bar *uitest.Window, name string) {
	t.Helper()
	if os.Getenv("WHITEROSE_CLOCKS_SHOTS") == "" {
		return
	}
	clocksShot(t, s, bar, name)
	// Captures use the real screenshot path, which raises an overlay toast
	// above notification actions. Let it leave before the next pointer click.
	if !uitest.WaitUntil(func() bool { return s.HasLayer("whiterose-toast") }) {
		t.Fatal("screenshot toast didn't appear")
	}
	if !uitest.WaitUntil(func() bool { return !s.HasLayer("whiterose-toast") }) {
		t.Fatal("screenshot toast didn't leave")
	}
}
func TestClocksTimersAndStopwatch(t *testing.T) {
	s := uitest.Start(t, uitest.Options{})
	s.Launch("whiterose-clocks", clocksBinary(s))
	app := s.WaitWindow("Clocks")
	app.Click(uitest.Named("New timer").Role("push button"))
	clocksShot(t, s, app, "new-timer")
	clocksField(app, "Timer minutes", "25")
	clocksField(app, "Timer name", "Focus")
	app.Click(uitest.Named("Start timer").Role("push button"))
	app.WaitFor(uitest.Named("Focus timer"))
	clocksShot(t, s, app, "timers")
	app.Click(uitest.Named("Pause").Role("push button"))
	app.WaitFor(uitest.Named("Resume").Role("push button"))
	before := clocksState(t, s).Timers[0].Remaining
	time.Sleep(250 * time.Millisecond)
	if got := clocksState(t, s).Timers[0].Remaining; got != before {
		t.Fatal("paused timer moved", before, got)
	}
	app.Click(uitest.Named("Resume").Role("push button"))
	app.WaitFor(uitest.Named("Pause").Role("push button"))
	app.Click(uitest.Named("Reset").Role("push button"))
	app.WaitFor(uitest.Named("Focus remaining").Described("25:00"))
	app.Click(uitest.Named("Stopwatch").Role("radio button"))
	app.Click(uitest.Named("Start").Role("push button"))
	app.Click(uitest.Named("Lap").Role("push button"))
	app.WaitFor(uitest.Named("Lap 1"))
	app.Click(uitest.Named("Pause").Role("push button"))
	clocksShot(t, s, app, "stopwatch")
	state := clocksState(t, s)
	if state.Stopwatch.Running || state.Stopwatch.Elapsed <= 0 || len(state.Stopwatch.Laps) != 1 {
		t.Fatal(state)
	}
	app.Click(uitest.Named("Close").Role("push button"))
	app.Closed()
	s.Launch("whiterose-clocks", clocksBinary(s), "--view", "stopwatch")
	app = s.WaitWindow("Clocks")
	app.WaitFor(uitest.Named("Lap 1"))
	app.Click(uitest.Named("Reset").Role("push button"))
	app.WaitFor(uitest.Named("No laps recorded"))
}
func TestClocksAlarmsWorldClocksAndLightTheme(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Settings: map[string]any{"appearance.theme": "light"}})
	s.Launch("whiterose-clocks", clocksBinary(s))
	app := s.WaitWindow("Clocks")
	app.Click(uitest.Named("Alarms").Role("radio button"))
	app.Click(uitest.Named("New alarm").Role("push button"))
	clocksField(app, "Alarm name", "Wake-up")
	clocksField(app, "Alarm hour", "07")
	clocksField(app, "Alarm minute", "30")
	app.Click(uitest.Named("Mo").Role("check box"))
	app.Click(uitest.Named("Fr").Role("check box"))
	clocksShot(t, s, app, "new-alarm-light")
	app.Click(uitest.Named("Save alarm").Role("push button"))
	app.WaitFor(uitest.Named("Wake-up alarm"))
	state := clocksState(t, s)
	if len(state.Alarms) != 1 || len(state.Alarms[0].Days) != 2 {
		t.Fatal(state)
	}
	app.Click(uitest.Named("Wake-up enabled").Role("check box"))
	if clocksState(t, s).Alarms[0].Enabled {
		t.Fatal("alarm didn't disable")
	}
	app.Click(uitest.Named("Edit Wake-up").Role("push button"))
	clocksField(app, "Alarm name", "Work")
	app.Click(uitest.Named("Save alarm").Role("push button"))
	app.WaitFor(uitest.Named("Work alarm"))
	if clocksState(t, s).Alarms[0].Enabled {
		t.Fatal("editing silently enabled a disabled alarm")
	}
	clocksShot(t, s, app, "alarms-light")
	app.Click(uitest.Named("World clock").Role("radio button"))
	app.Click(uitest.Named("Add city").Role("push button"))
	clocksField(app, "Search cities", "Asia/Tokyo")
	app.Click(uitest.Named("Tokyo, Asia/Tokyo").Role("radio button"))
	clocksShot(t, s, app, "add-city-light")
	app.Click(uitest.Named("Add selected city").Role("push button"))
	app.WaitFor(uitest.Named("Tokyo clock"))
	clocksShot(t, s, app, "world-light")
	app.Click(uitest.Named("Remove Tokyo").Role("push button"))
	app.Gone(uitest.Named("Tokyo clock"))
	app.Click(uitest.Named("Alarms").Role("radio button"))
	app.Click(uitest.Named("Delete Work").Role("push button"))
	app.WaitFor(uitest.Named("No alarms"))
}

func TestClocksNarrowDialogsAndMultipleTimers(t *testing.T) {
	s := uitest.Start(t, uitest.Options{})
	s.Request("clocks_command", map[string]any{"action": "timer_add", "name": "Focus", "duration": 1500000})
	s.Request("clocks_command", map[string]any{"action": "timer_add", "name": "Tea", "duration": 300000})
	s.Launch("whiterose-clocks", clocksBinary(s))
	app := s.WaitWindow("Clocks")
	app.WaitFor(uitest.Named("Focus timer"))
	app.WaitFor(uitest.Named("Tea timer"))
	clocksShot(t, s, app, "multiple-timers")
	g := app.Geometry()
	x, y := float64(g[0]+g[2]-4), float64(g[1]+g[3]-4)
	s.Drag(x, y, x-290, y-250)
	if !uitest.WaitUntil(func() bool { g := app.Geometry(); return g[2] < 500 && g[3] < 450 }) {
		t.Fatal(app.Geometry())
	}
	app.Click(uitest.Named("New timer").Role("push button"))
	clocksField(app, "Timer minutes", "00")
	app.Click(uitest.Named("Start timer").Role("push button"))
	app.WaitFor(uitest.Named("Choose a duration greater than zero, up to 7 days"))
	clocksShot(t, s, app, "narrow-timer-validation")
	app.Click(uitest.Named("Cancel").Role("push button"))
	app.Click(uitest.Named("Alarms").Role("radio button"))
	app.Click(uitest.Named("New alarm").Role("push button"))
	clocksField(app, "Alarm name", "Narrow")
	app.Click(uitest.Named("Save alarm").Role("push button"))
	app.WaitFor(uitest.Named("Narrow alarm"))
	clocksShot(t, s, app, "narrow-alarm")
	app.Click(uitest.Named("World clock").Role("radio button"))
	app.Click(uitest.Named("Add city").Role("push button"))
	clocksField(app, "Search cities", "Asia/Kolkata")
	app.Click(uitest.Named("Kolkata, Asia/Kolkata").Role("radio button"))
	app.Click(uitest.Named("Add selected city").Role("push button"))
	app.Click(uitest.Named("Add city").Role("push button"))
	clocksField(app, "Search cities", "Asia/Kolkata")
	app.Click(uitest.Named("Kolkata, Asia/Kolkata").Role("radio button"))
	app.Click(uitest.Named("Add selected city").Role("push button"))
	app.WaitFor(uitest.Named("That timezone is already in your world clocks"))
	app.Click(uitest.Named("Cancel").Role("push button"))
	app.Click(uitest.Named("Remove Kolkata").Role("push button"))
}

func clocksChoice(app *uitest.Window, label, value string) {
	app.Click(uitest.Named(label).Role("combo box"))
	// Qt reports popup entries relative to the popup, not the app window.
	// Press the entry through AT-SPI rather than clicking an incorrect offset.
	app.Activate(uitest.Named(value).Role("menu item"))
	app.WaitFor(uitest.Named(label).Role("combo box").Described(value))
}

func TestClocksAlarmSoundAndDefaultSettings(t *testing.T) {
	s := uitest.Start(t, uitest.Options{})
	s.Request("clocks_command", map[string]any{"action": "alarm_save", "name": "Existing", "hour": 7, "enabled": true})
	s.Launch("whiterose-clocks", clocksBinary(s), "--view", "alarms")
	app := s.WaitWindow("Clocks")
	app.Click(uitest.Named("Defaults").Role("push button"))
	clocksChoice(app, "Alarm chime", "Soft bells")
	app.Click(uitest.Named("Alarm volume").Role("slider"))
	clocksChoice(app, "Snooze duration", "15 minutes")
	clocksChoice(app, "Ring duration", "2 minutes")
	clocksChoice(app, "Fade-in duration", "Fade in over 10 seconds")
	clocksShot(t, s, app, "alarm-defaults")
	app.Click(uitest.Named("Save defaults").Role("push button"))
	state := clocksState(t, s)
	if state.Defaults.Sound != "bells" || state.Defaults.SnoozeDuration != 900000 || state.Defaults.RingDuration != 120000 || state.Defaults.FadeDuration != 10000 || state.Alarms[0].Options != clocks.DefaultAlarmOptions() {
		t.Fatal(state)
	}
	app.Click(uitest.Named("New alarm").Role("push button"))
	clocksField(app, "Alarm name", "Custom wake")
	app.Click(uitest.Named("Sound & behavior").Role("push button"))
	app.WaitFor(uitest.Named("Alarm chime").Described("Soft bells"))
	clocksChoice(app, "Alarm chime", "Pulse")
	clocksChoice(app, "Snooze duration", "10 minutes")
	clocksShot(t, s, app, "alarm-sound-options")
	app.Click(uitest.Named("Done").Role("push button"))
	app.Click(uitest.Named("Save alarm").Role("push button"))
	app.WaitFor(uitest.Named("Custom wake alarm"))
	state = clocksState(t, s)
	if state.Alarms[1].Options.Sound != "pulse" || state.Alarms[1].Options.SnoozeDuration != 600000 || state.Alarms[1].Options.Volume != state.Defaults.Volume {
		t.Fatal(state)
	}
	app.Click(uitest.Named("Edit Custom wake").Role("push button"))
	app.Click(uitest.Named("Sound & behavior").Role("push button"))
	clocksChoice(app, "Alarm chime", "Dawn")
	app.Click(uitest.Named("Cancel").Role("push button"))
	app.Click(uitest.Named("Save alarm").Role("push button"))
	app.WaitFor(uitest.Named("Custom wake alarm"))
	if clocksState(t, s).Alarms[1].Options.Sound != "pulse" {
		t.Fatal("cancel kept draft changes")
	}
}

func TestClocksImportedAlarmAudioPersistsWithoutOriginal(t *testing.T) {
	if _, err := exec.LookPath("ffmpeg"); err != nil {
		t.Skip("ffmpeg not installed")
	}
	s := uitest.Start(t, uitest.Options{Settings: map[string]any{"appearance.theme": "light"}})
	pcm, _ := clocks.SoundPCM("dawn", "")
	path := filepath.Join(s.Home, "Wake up ü.wav")
	if err := os.WriteFile(path, clocks.WAV(pcm), 0600); err != nil {
		t.Fatal(err)
	}
	raw := s.Request("clocks_import_sound", map[string]any{"path": (&url.URL{Scheme: "file", Path: path}).String()})
	var imported struct {
		Sound string `json:"sound"`
	}
	if err := json.Unmarshal(raw, &imported); err != nil || imported.Sound == "" {
		t.Fatal(string(raw), err)
	}
	if err := os.Remove(path); err != nil {
		t.Fatal(err)
	}
	s.Launch("whiterose-clocks", clocksBinary(s), "--view", "alarms")
	app := s.WaitWindow("Clocks")
	app.Click(uitest.Named("Defaults").Role("push button"))
	clocksChoice(app, "Alarm chime", "Wake up ü")
	clocksShot(t, s, app, "imported-alarm-audio-light")
	app.Click(uitest.Named("Save defaults").Role("push button"))
	if clocksState(t, s).Defaults.Sound != imported.Sound {
		t.Fatal("custom sound wasn't saved")
	}
	app.Click(uitest.Named("Close").Role("push button"))
	app.Closed()
	s.Launch("whiterose-clocks", clocksBinary(s), "--view", "alarms")
	app = s.WaitWindow("Clocks")
	app.Click(uitest.Named("New alarm").Role("push button"))
	clocksField(app, "Alarm name", "Imported")
	app.Click(uitest.Named("Save alarm").Role("push button"))
	app.WaitFor(uitest.Named("Imported alarm"))
	if clocksState(t, s).Alarms[0].Options.Sound != imported.Sound {
		t.Fatal("new alarm didn't inherit the imported sound")
	}
}
func TestClocksIslandReopensAppAndTimerAlertsWithAppClosed(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Shell: true, Settings: map[string]any{"quick.doNotDisturb": true}})
	bar := s.Bar()
	s.Request("clocks_command", map[string]any{"action": "timer_add", "name": "Focus", "duration": 1500000})
	entry := uitest.Named("Clocks: Focus").Role("push button")
	bar.WaitFor(entry)
	clocksShellShot(t, s, bar, "island-timer")
	bar.Click(entry)
	app := s.WaitWindow("Clocks")
	app.WaitFor(uitest.Named("Timers").Role("radio button"))
	app.Click(uitest.Named("Stopwatch").Role("radio button"))
	bar.Click(entry)
	if !app.WaitFor(uitest.Named("Timers").Role("radio button")).Checked() {
		t.Fatal("island didn't select Timers")
	}
	if s.AppWindowCount("whiterose-clocks") != 1 {
		t.Fatal("island opened a duplicate window")
	}
	app.Click(uitest.Named("Close").Role("push button"))
	app.Closed()
	s.Request("clocks_command", map[string]any{"action": "timer_add", "name": "Tea", "duration": 500})
	bar.WaitFor(uitest.Named("Restart timer").Role("push button"))
	clocksShellShot(t, s, bar, "timer-alert")
	if len(clocksState(t, s).Alerts) != 1 {
		t.Fatal("closed app's timer didn't alert")
	}
	bar.Click(uitest.Named("Dismiss").Role("push button"))
	if !uitest.WaitUntil(func() bool { return len(clocksState(t, s).Alerts) == 0 }) {
		t.Fatal("notification didn't dismiss the daemon alert")
	}
}
