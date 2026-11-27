package clocks

import (
	"bytes"
	"context"
	"encoding/binary"
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestAlarmOptionsDefaultsAreCopiedAndEditsKeepExistingOptions(t *testing.T) {
	s, _ := rig(t)
	apply(t, s, Command{Action: "alarm_save", Name: "Old", Hour: 7, Enabled: true})
	custom := DefaultAlarmOptions()
	custom.Sound = "bells"
	custom.Volume = 35
	custom.SnoozeDuration = 900000
	custom.RingDuration = 120000
	custom.FadeDuration = 10000
	state := apply(t, s, Command{Action: "defaults_save", Options: &custom})
	if state.Alarms[0].Options != DefaultAlarmOptions() {
		t.Fatal("defaults changed an existing alarm")
	}
	state = apply(t, s, Command{Action: "alarm_save", Name: "New", Hour: 8, Enabled: true})
	if state.Alarms[1].Options != custom {
		t.Fatal(state.Alarms[1])
	}
	state = apply(t, s, Command{Action: "alarm_save", ID: state.Alarms[1].ID, Name: "Edited", Hour: 9, Enabled: true})
	if state.Alarms[1].Options != custom {
		t.Fatal("legacy edit discarded settings")
	}
	reopened, err := New(s.path, time.UTC, s.now, nil)
	if err != nil || reopened.Snapshot().Defaults != custom || reopened.Snapshot().Alarms[1].Options != custom {
		t.Fatal(err)
	}
}

func TestAlarmOptionsMigrationDoesNotUseNewDefaultsForOldAlarms(t *testing.T) {
	s, n := rig(t)
	state := apply(t, s, Command{Action: "alarm_save", Name: "Old", Hour: 7, Enabled: true})
	customDefaults := DefaultAlarmOptions()
	customDefaults.Sound = "bells"
	apply(t, s, Command{Action: "defaults_save", Options: &customDefaults})
	data, err := os.ReadFile(s.path)
	if err != nil {
		t.Fatal(err)
	}
	// A file written by the initial Clocks build has no options or defaults.
	optionsJSON, _ := json.Marshal(DefaultAlarmOptions())
	data = bytes.ReplaceAll(data, append([]byte(`"options":`), optionsJSON...), []byte(`"legacy_options":{}`))
	if err = os.WriteFile(s.path, data, 0600); err != nil {
		t.Fatal(err)
	}
	reopened, err := New(s.path, time.UTC, func() Stamp { return *n }, nil)
	if err != nil {
		t.Fatal(err)
	}
	if reopened.Snapshot().Alarms[0].ID != state.Alarms[0].ID || reopened.Snapshot().Alarms[0].Options != DefaultAlarmOptions() || reopened.Snapshot().Defaults != customDefaults {
		t.Fatal(reopened.Snapshot())
	}
}

func TestAlarmRingLimitAndSnoozeSurviveRestartAndWallChanges(t *testing.T) {
	s, n := rig(t)
	o := DefaultAlarmOptions()
	o.SnoozeDuration = 600000
	o.RingDuration = 30000
	o.FadeDuration = 10000
	state := apply(t, s, Command{Action: "alarm_save", Name: "Wake", Hour: 16, Enabled: true, Options: &o})
	n.Wall = state.Alarms[0].Next
	s.Tick()
	n.Boot += 10000
	n.Wall += 10000
	// A wall-clock adjustment cannot consume the ring limit or snooze.
	n.Wall += 3600000
	s.Tick()
	reopened, err := New(s.path, time.UTC, func() Stamp { return *n }, nil)
	if err != nil {
		t.Fatal(err)
	}
	if got := reopened.Snapshot().Alerts[0]; got.Elapsed != 10000 || got.Silenced {
		t.Fatal(got)
	}
	n.Boot += 20000
	n.Wall += 20000
	reopened.Tick()
	state = reopened.Snapshot()
	if len(state.Alerts) != 1 || !state.Alerts[0].Silenced {
		t.Fatal(state)
	}
	state = apply(t, reopened, Command{Action: "alert_snooze", ID: state.Alerts[0].ID})
	if state.Alarms[0].Snooze != n.Wall+600000 {
		t.Fatal(state)
	}
	n.Boot += 600000
	n.Wall += 600000
	reopened.Tick()
	if got := reopened.Snapshot().Alerts[0]; got.Silenced || got.Elapsed != 0 || got.Options != o {
		t.Fatal(got)
	}
}

func TestInvalidAlarmOptionsDoNotChangeState(t *testing.T) {
	s, _ := rig(t)
	for _, change := range []func(*AlarmOptions){
		func(o *AlarmOptions) { o.Sound = "missing" }, func(o *AlarmOptions) { o.Volume = -1 }, func(o *AlarmOptions) { o.Volume = 101 },
		func(o *AlarmOptions) { o.SnoozeDuration = 0 }, func(o *AlarmOptions) { o.SnoozeDuration = 3600001 },
		func(o *AlarmOptions) { o.RingDuration = -1 }, func(o *AlarmOptions) { o.RingDuration = 999 }, func(o *AlarmOptions) { o.FadeDuration = 60001 },
		func(o *AlarmOptions) { o.RingDuration = 10000; o.FadeDuration = 30000 },
	} {
		o := DefaultAlarmOptions()
		change(&o)
		if _, err := s.Apply(Command{Action: "defaults_save", Options: &o}); err == nil {
			t.Fatal(o)
		}
		if s.Snapshot().Defaults != DefaultAlarmOptions() {
			t.Fatal("invalid options persisted")
		}
	}
}

func TestBuiltinChimesAreDistinctAndGainIsBounded(t *testing.T) {
	var previous [][]byte
	for _, sound := range BuiltinSounds() {
		pcm, err := SoundPCM(sound.ID, "")
		if err != nil {
			t.Fatal(err)
		}
		if len(pcm) != AudioRate*2 {
			t.Fatal(len(pcm))
		}
		for _, old := range previous {
			if bytes.Equal(old, pcm) {
				t.Fatal("identical chimes")
			}
		}
		previous = append(previous, pcm)
	}
	pcm := []byte{0x10, 0x27, 0xf0, 0xd8} // +10000, -10000
	o := DefaultAlarmOptions()
	o.Volume = 50
	out := Gain(pcm, o, 0)
	if int16(binary.LittleEndian.Uint16(out)) != 5000 || int16(binary.LittleEndian.Uint16(out[2:])) != -5000 || bytes.Equal(out, pcm) {
		t.Fatal(out)
	}
	o.Volume = 0
	if out = Gain(pcm, o, 0); !bytes.Equal(out, make([]byte, len(pcm))) {
		t.Fatal(out)
	}
	o.Volume = 100
	o.FadeDuration = 10000
	if int16(binary.LittleEndian.Uint16(Gain(pcm, o, 5000))) != 5000 || int16(binary.LittleEndian.Uint16(Gain(pcm, o, 0))) != 0 {
		t.Fatal("fade gain")
	}
}

func TestCustomAudioIsPrivateDeduplicatedAndIndependentOfSource(t *testing.T) {
	if _, err := exec.LookPath("ffmpeg"); err != nil {
		t.Skip("ffmpeg not installed")
	}
	source := filepath.Join(t.TempDir(), "My alarm ü.wav")
	pcm, _ := SoundPCM("bells", "")
	if err := os.WriteFile(source, WAV(pcm), 0600); err != nil {
		t.Fatal(err)
	}
	dir := filepath.Join(t.TempDir(), "sounds")
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	first, err := ImportSound(ctx, source, dir)
	if err != nil {
		t.Fatal(err)
	}
	second, err := ImportSound(ctx, source, dir)
	if err != nil || first.ID != second.ID || first.Name != "My alarm ü" {
		t.Fatal(first, second, err)
	}
	os.Remove(source)
	got, err := SoundPCM(first.ID, dir)
	if err != nil || !bytes.Equal(got, pcm) {
		t.Fatal(err)
	}
	path := filepath.Join(dir, strings.TrimPrefix(first.ID, "custom:")+".wav")
	info, err := os.Stat(path)
	if err != nil || info.Mode().Perm() != 0600 {
		t.Fatal(info, err)
	}
	for _, bad := range []string{"relative.wav", "https://example.com/audio.mp3", "file://remote/path.wav", t.TempDir()} {
		if _, err := ImportSound(ctx, bad, dir); err == nil {
			t.Fatal(bad)
		}
	}
	s, _ := rig(t)
	state, err := s.RegisterSound(first)
	if err != nil || len(state.Sounds) != 5 {
		t.Fatal(err)
	}
	o := DefaultAlarmOptions()
	o.Sound = first.ID
	apply(t, s, Command{Action: "alarm_save", Name: "Custom", Hour: 7, Options: &o})
}
