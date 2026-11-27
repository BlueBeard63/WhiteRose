package clocks

import (
	"errors"
	"regexp"
)

// Durations use milliseconds, like the rest of the Clocks protocol.
// A zero ring duration keeps sounding until acknowledged.
type AlarmOptions struct {
	Sound          string `json:"sound"`
	Volume         int    `json:"volume"`
	SnoozeDuration int64  `json:"snooze_duration"`
	RingDuration   int64  `json:"ring_duration"`
	FadeDuration   int64  `json:"fade_duration"`
}

type Sound struct {
	ID     string `json:"id"`
	Name   string `json:"name"`
	Custom bool   `json:"custom,omitempty"`
}

func DefaultAlarmOptions() AlarmOptions {
	return AlarmOptions{Sound: "chime", Volume: 100, SnoozeDuration: 300000}
}

func BuiltinSounds() []Sound {
	return []Sound{{ID: "chime", Name: "Classic"}, {ID: "bells", Name: "Soft bells"}, {ID: "dawn", Name: "Dawn"}, {ID: "pulse", Name: "Pulse"}}
}

var customSoundID = regexp.MustCompile(`^custom:[a-f0-9]{64}$`)

func IsCustomSound(id string) bool { return customSoundID.MatchString(id) }

func (s *Service) validOptions(o AlarmOptions) error {
	known := false
	for _, sound := range append(BuiltinSounds(), s.state.Sounds...) {
		if sound.ID == o.Sound {
			known = true
			break
		}
	}
	if !known {
		return errors.New("Choose an available alarm chime")
	}
	if o.Volume < 0 || o.Volume > 100 {
		return errors.New("Volume must be between 0 and 100 percent")
	}
	if o.SnoozeDuration < 60000 || o.SnoozeDuration > 3600000 {
		return errors.New("Choose a snooze duration from 1 to 60 minutes")
	}
	if o.RingDuration < 0 || o.RingDuration > 3600000 || o.RingDuration > 0 && o.RingDuration < 1000 {
		return errors.New("Choose a ring duration from 1 second to 1 hour, or until dismissed")
	}
	if o.FadeDuration < 0 || o.FadeDuration > 60000 || o.RingDuration > 0 && o.FadeDuration > o.RingDuration {
		return errors.New("Fade-in must be at most 60 seconds and no longer than the ring duration")
	}
	return nil
}

func (s *Service) ValidateOptions(o AlarmOptions) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.validOptions(o)
}

// RegisterSound is used after the daemon has converted and privately copied a
// file. Options store a content ID, never an arbitrary path to play later.
func (s *Service) RegisterSound(sound Sound) (State, error) {
	return s.Apply(Command{Action: "sound_register", ID: sound.ID, Name: sound.Name})
}

func (s *Service) newAlarmAlert(a Alarm, n Stamp, message string) Alert {
	return Alert{ID: id(), Kind: "alarm", ItemID: a.ID, Name: a.Name, Time: n.Wall, Message: message, Options: a.Options, soundBoot: n.Boot}
}

func alertElapsed(a Alert, n Stamp) int64 { return a.Elapsed + max(int64(0), n.Boot-a.soundBoot) }
