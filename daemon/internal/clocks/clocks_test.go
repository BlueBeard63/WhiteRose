package clocks

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestRunningClocksRecoverThroughDaemonRestart(t *testing.T) {
	s, n := rig(t)
	state := apply(t, s, Command{Action: "timer_add", Name: "Tea", Duration: 5000})
	apply(t, s, Command{Action: "stopwatch_start"})
	n.Boot += 2000
	n.Wall += 2000
	restarted, err := New(s.path, time.UTC, func() Stamp { return *n }, nil)
	if err != nil {
		t.Fatal(err)
	}
	if got := restarted.Snapshot(); got.Timers[0].Remaining != 3000 || got.Stopwatch.Elapsed != 2000 {
		t.Fatal(got)
	}
	n.Boot += 4000
	n.Wall += 4000
	if err := restarted.Tick(); err != nil {
		t.Fatal(err)
	}
	got := restarted.Snapshot()
	if len(got.Alerts) != 1 || got.Alerts[0].ItemID != state.Timers[0].ID {
		t.Fatal(got)
	}
	restarted, err = New(s.path, time.UTC, func() Stamp { return *n }, nil)
	if err != nil {
		t.Fatal(err)
	}
	restarted.Tick()
	if len(restarted.Snapshot().Alerts) != 1 {
		t.Fatal("restart lost or duplicated an alert")
	}
}

func TestWallChangesRebaseSnapshotsAndPersistedDeadlines(t *testing.T) {
	s, n := rig(t)
	apply(t, s, Command{Action: "timer_add", Name: "Tea", Duration: 60000})
	apply(t, s, Command{Action: "stopwatch_start"})
	before := s.Snapshot().Revision
	n.Boot += 5000
	n.Wall += 3605000
	if err := s.Tick(); err != nil {
		t.Fatal(err)
	}
	state := s.Snapshot()
	if state.Revision <= before || state.Timers[0].Remaining != 55000 || state.Timers[0].Deadline != n.Wall+55000 || state.Stopwatch.Elapsed != 5000 {
		t.Fatal(state)
	}
	restarted, err := New(s.path, time.UTC, func() Stamp { return *n }, nil)
	if err != nil {
		t.Fatal(err)
	}
	if state := restarted.Snapshot(); state.Timers[0].Remaining != 55000 || state.Stopwatch.Elapsed != 5000 {
		t.Fatal(state)
	}
}

func TestTimezoneChangesRescheduleAlarmsButNotElapsedClocks(t *testing.T) {
	s, n := rig(t)
	apply(t, s, Command{Action: "alarm_save", Name: "Wake", Hour: 7, Minute: 30, Enabled: true})
	apply(t, s, Command{Action: "timer_add", Name: "Tea", Duration: 60000})
	apply(t, s, Command{Action: "stopwatch_start"})
	loc, err := time.LoadLocation("Asia/Tokyo")
	if err != nil {
		t.Fatal(err)
	}
	if err := s.SetLocation(loc); err != nil {
		t.Fatal(err)
	}
	state := s.Snapshot()
	if got := time.UnixMilli(state.Alarms[0].Next).In(loc).Format("2006-01-02 15:04"); got != "2026-11-27 07:30" {
		t.Fatal(got)
	}
	if state.Cities[0].Zone != "Asia/Tokyo" || state.Cities[0].Offset != 32400 || state.Timers[0].Remaining != 60000 || state.Stopwatch.Elapsed != 0 {
		t.Fatal(state)
	}
	n.Wall = state.Alarms[0].Next
	s.Tick()
	if len(s.Snapshot().Alerts) != 1 {
		t.Fatal(s.Snapshot())
	}
}

func TestRestartInAnotherTimezoneReschedulesFutureAlarms(t *testing.T) {
	s, n := rig(t)
	apply(t, s, Command{Action: "alarm_save", Name: "Wake", Hour: 7, Minute: 30, Enabled: true})
	loc, err := time.LoadLocation("Asia/Tokyo")
	if err != nil {
		t.Fatal(err)
	}
	restarted, err := New(s.path, loc, func() Stamp { return *n }, nil)
	if err != nil {
		t.Fatal(err)
	}
	if got := time.UnixMilli(restarted.Snapshot().Alarms[0].Next).In(loc).Format("2006-01-02 15:04"); got != "2026-11-27 07:30" {
		t.Fatal(got)
	}
}

func TestSnoozeUsesBootTimeAndSurvivesRestart(t *testing.T) {
	s, n := rig(t)
	state := apply(t, s, Command{Action: "alarm_save", Name: "Wake", Hour: 16, Enabled: true})
	n.Wall = state.Alarms[0].Next
	s.Tick()
	state = apply(t, s, Command{Action: "alert_snooze", ID: s.Snapshot().Alerts[0].ID})
	n.Boot += 60000
	n.Wall -= 3600000
	s.Tick()
	if got := s.Snapshot(); len(got.Alerts) != 0 || got.Alarms[0].Snooze != n.Wall+240000 {
		t.Fatal(got)
	}
	restarted, err := New(s.path, time.UTC, func() Stamp { return *n }, nil)
	if err != nil {
		t.Fatal(err)
	}
	n.Boot += 240000
	n.Wall += 240000
	restarted.Tick()
	if got := restarted.Snapshot(); len(got.Alerts) != 1 || got.Alerts[0].Kind != "alarm" || len(got.Timers) != 0 {
		t.Fatal(got)
	}
}

func TestDueAlertsDeliverOnSaveFailureAndRetryPersistence(t *testing.T) {
	s, n := rig(t)
	apply(t, s, Command{Action: "timer_add", Name: "Tea", Duration: 1000})
	path := s.path
	s.path = filepath.Join(path, "impossible", "state.json")
	n.Boot += 1000
	n.Wall += 1000
	if err := s.Tick(); err == nil {
		t.Fatal("expected save failure")
	}
	if len(s.Snapshot().Alerts) != 1 || !s.dirty {
		t.Fatal(s.Snapshot())
	}
	s.path = path
	if err := s.Tick(); err != nil {
		t.Fatal(err)
	}
	restarted, err := New(path, time.UTC, func() Stamp { return *n }, nil)
	if err != nil {
		t.Fatal(err)
	}
	if len(restarted.Snapshot().Alerts) != 1 || s.dirty {
		t.Fatal(restarted.Snapshot())
	}
}

func TestCorruptStateIsNotOverwritten(t *testing.T) {
	path := filepath.Join(t.TempDir(), "state.json")
	data := []byte(`{"timers":[`)
	if err := os.WriteFile(path, data, 0600); err != nil {
		t.Fatal(err)
	}
	if _, err := New(path, time.UTC, Now, nil); err == nil {
		t.Fatal("accepted corrupt state")
	}
	saved, err := os.ReadFile(path)
	if err != nil || !bytes.Equal(saved, data) {
		t.Fatal("corrupt state was overwritten", err)
	}
}

func rig(t *testing.T) (*Service, *Stamp) {
	t.Helper()
	n := &Stamp{time.Date(2026, 11, 26, 15, 26, 0, 0, time.UTC).UnixMilli(), 1000000}
	s, err := New(filepath.Join(t.TempDir(), "state.json"), time.UTC, func() Stamp { return *n }, nil)
	if err != nil {
		t.Fatal(err)
	}
	return s, n
}
func apply(t *testing.T, s *Service, c Command) State {
	t.Helper()
	state, err := s.Apply(c)
	if err != nil {
		t.Fatal(err)
	}
	return state
}
func TestTimersPauseResumeResetRestartAndPersistence(t *testing.T) {
	s, n := rig(t)
	state := apply(t, s, Command{Action: "timer_add", Name: "Tea", Duration: 5000})
	id := state.Timers[0].ID
	n.Boot += 2000
	n.Wall += 2000
	state = apply(t, s, Command{Action: "timer_pause", ID: id})
	if state.Timers[0].Remaining != 3000 || state.Timers[0].Running {
		t.Fatal(state)
	}
	n.Boot += 100000
	n.Wall += 100000
	apply(t, s, Command{Action: "timer_resume", ID: id})
	n.Boot += 3000
	n.Wall += 3000
	if err := s.Tick(); err != nil {
		t.Fatal(err)
	}
	state = s.Snapshot()
	if len(state.Alerts) != 1 || state.Timers[0].Remaining != 0 {
		t.Fatal(state)
	}
	s.Tick()
	if len(s.Snapshot().Alerts) != 1 {
		t.Fatal("alert fired twice")
	}
	apply(t, s, Command{Action: "alert_restart", ID: state.Alerts[0].ID})
	if len(s.Snapshot().Alerts) != 0 || !s.Snapshot().Timers[0].Running {
		t.Fatal(s.Snapshot())
	}
	apply(t, s, Command{Action: "timer_reset", ID: id})
	if s.Snapshot().Timers[0].Running || s.Snapshot().Timers[0].Remaining != 5000 {
		t.Fatal(s.Snapshot())
	}
	reopened, err := New(s.path, time.UTC, func() Stamp { return *n }, nil)
	if err != nil || reopened.Snapshot().Timers[0].Remaining != 5000 {
		t.Fatal(err)
	}
	info, _ := os.Stat(s.path)
	if info.Mode().Perm() != 0o600 {
		t.Fatal(info.Mode())
	}
}
func TestElapsedUsesBootTimeIncludingSuspendNotTheWallClock(t *testing.T) {
	s, n := rig(t)
	apply(t, s, Command{Action: "timer_add", Name: "Timer", Duration: 60000})
	apply(t, s, Command{Action: "stopwatch_start"})
	n.Boot += 12000
	n.Wall -= 3600000
	state := s.Snapshot()
	if state.Timers[0].Remaining != 48000 || state.Stopwatch.Elapsed != 12000 {
		t.Fatal(state)
	}
	apply(t, s, Command{Action: "stopwatch_lap"})
	n.Boot += 5000
	state = apply(t, s, Command{Action: "stopwatch_pause"})
	if state.Stopwatch.Elapsed != 17000 || len(state.Stopwatch.Laps) != 1 || state.Stopwatch.Laps[0] != 12000 {
		t.Fatal(state)
	}
	n.Boot += 60000
	s.Tick()
	if len(s.Snapshot().Alerts) != 1 {
		t.Fatal("timer didn't expire through suspend")
	}
}
func TestAlarmRecurrenceOneShotSnoozeAndDismiss(t *testing.T) {
	s, n := rig(t)
	state := apply(t, s, Command{Action: "alarm_save", Name: "Wake", Hour: 7, Minute: 30, Enabled: true, Days: []int{0, 1, 2, 3, 4}})
	if got := time.UnixMilli(state.Alarms[0].Next).Format("Mon 15:04"); got != "Fri 07:30" {
		t.Fatal(got)
	}
	n.Wall = state.Alarms[0].Next
	s.Tick()
	state = s.Snapshot()
	if len(state.Alerts) != 1 || time.UnixMilli(state.Alarms[0].Next).Weekday() != time.Monday {
		t.Fatal(state)
	}
	apply(t, s, Command{Action: "alert_snooze", ID: state.Alerts[0].ID})
	n.Wall += 300000
	n.Boot += 300000
	s.Tick()
	state = s.Snapshot()
	if len(state.Alerts) != 1 || state.Alerts[0].Kind != "alarm" {
		t.Fatal(state)
	}
	apply(t, s, Command{Action: "alert_dismiss", ID: state.Alerts[0].ID})
	state = apply(t, s, Command{Action: "alarm_save", Name: "Once", Hour: 8, Enabled: true})
	n.Wall = state.Alarms[1].Next
	s.Tick()
	if s.Snapshot().Alarms[1].Enabled {
		t.Fatal("one-shot alarm wasn't disabled")
	}
}
func TestNextAlarmHandlesDSTAndDoesNotRingTwice(t *testing.T) {
	loc, err := time.LoadLocation("Europe/London")
	if err != nil {
		t.Fatal(err)
	}
	a := Alarm{Name: "DST", Hour: 1, Minute: 30, Days: []int{6}}
	next := Next(a, time.Date(2026, 3, 28, 18, 0, 0, 0, loc), loc)
	if next.Day() != 5 || next.Month() != time.April || next.Hour() != 1 {
		t.Fatal(next)
	}
	first := Next(a, time.Date(2026, 10, 24, 18, 0, 0, 0, loc), loc)
	after := Next(a, first.Add(time.Second), loc)
	if after.Sub(first) < 6*24*time.Hour {
		t.Fatal("fall DST alarm repeated", first, after)
	}
}
func TestValidationAndFailedSavesDoNotChangeState(t *testing.T) {
	s, _ := rig(t)
	for _, c := range []Command{
		{Action: "timer_add", Name: "T", Duration: 0},
		{Action: "timer_add", Name: "T", Duration: MaxDuration + 1},
		{Action: "alarm_save", Name: "A", Hour: 24},
		{Action: "alarm_save", Name: "A", Days: []int{7}},
		{Action: "alarm_save", Name: "A", Days: []int{1, 1}},
		{Action: "city_add", Name: "Bad", Zone: "../../etc/passwd"},
		{Action: "stopwatch_lap"},
		{Action: "timer_delete", ID: "missing"},
	} {
		if _, err := s.Apply(c); err == nil {
			t.Fatal(c)
		}
	}
	s.path = filepath.Join(s.path, "not-a-directory", "state.json")
	if err := os.WriteFile(filepath.Dir(filepath.Dir(s.path)), []byte("file"), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := s.Apply(Command{Action: "timer_add", Name: "T", Duration: 1000}); err == nil {
		t.Fatal("save should fail")
	}
	if len(s.Snapshot().Timers) != 0 {
		t.Fatal("failed mutation was kept")
	}
}
func TestCityOffsetsAndSnapshotsAreIndependent(t *testing.T) {
	s, _ := rig(t)
	state := apply(t, s, Command{Action: "city_add", Name: "Tokyo", Zone: "Asia/Tokyo"})
	if len(state.Cities) != 2 || state.Cities[1].Offset != 9*3600 || !state.Cities[0].Local {
		t.Fatal(state)
	}
	state.Cities[1].Name = "mutated"
	if s.Snapshot().Cities[1].Name != "Tokyo" {
		t.Fatal("snapshot shared state")
	}
	if _, err := s.Apply(Command{Action: "city_add", Name: "Tokyo", Zone: "Asia/Tokyo"}); err == nil {
		t.Fatal("duplicate timezone")
	}
	if got := SearchZones("Asia/Tokyo"); len(got) != 1 || got[0].Zone != "Asia/Tokyo" {
		t.Fatal(got)
	}
}
