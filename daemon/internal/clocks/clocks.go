// Package clocks owns alarms, timers and the stopwatch independently of the UI.
// Deadlines survive app closes; elapsed time uses CLOCK_BOOTTIME while running,
// so suspend counts, but changing the wall clock doesn't change a timer.
package clocks

import (
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"time"
	"unicode/utf8"

	"golang.org/x/sys/unix"
)

const MaxItems = 100
const MaxDuration = 7 * 24 * 60 * 60 * 1000

type Stamp struct{ Wall, Boot int64 }

func Now() Stamp {
	var ts unix.Timespec
	if unix.ClockGettime(unix.CLOCK_BOOTTIME, &ts) != nil {
		return Stamp{time.Now().UnixMilli(), time.Now().UnixMilli()}
	}
	return Stamp{time.Now().UnixMilli(), ts.Sec*1000 + ts.Nsec/1e6}
}

type Timer struct {
	ID        string `json:"id"`
	Name      string `json:"name"`
	Duration  int64  `json:"duration"`
	Remaining int64  `json:"remaining"`
	Deadline  int64  `json:"deadline"`
	Running   bool   `json:"running"`
	endBoot   int64
}
type Alarm struct {
	ID         string `json:"id"`
	Name       string `json:"name"`
	Hour       int    `json:"hour"`
	Minute     int    `json:"minute"`
	Days       []int  `json:"days"` // Monday=0 ... Sunday=6; empty means once.
	Enabled    bool   `json:"enabled"`
	Next       int64  `json:"next"`
	Snooze     int64  `json:"snooze"`
	snoozeBoot int64
	Options    AlarmOptions `json:"options"`
}
type Stopwatch struct {
	Running bool    `json:"running"`
	Elapsed int64   `json:"elapsed"`
	Started int64   `json:"started"`
	Laps    []int64 `json:"laps"`
	boot    int64
}
type City struct {
	Name    string `json:"name"`
	Country string `json:"country"`
	Zone    string `json:"zone"`
	Offset  int    `json:"offset"`
	Local   bool   `json:"local,omitempty"`
}
type Alert struct {
	ID        string       `json:"id"`
	Kind      string       `json:"kind"`
	ItemID    string       `json:"item_id"`
	Name      string       `json:"name"`
	Time      int64        `json:"time"`
	Message   string       `json:"message"`
	Options   AlarmOptions `json:"options"`
	Elapsed   int64        `json:"elapsed"`
	Silenced  bool         `json:"silenced"`
	soundBoot int64
}
type State struct {
	Revision  uint64       `json:"revision"`
	Now       int64        `json:"now"`
	Timers    []Timer      `json:"timers"`
	Alarms    []Alarm      `json:"alarms"`
	Stopwatch Stopwatch    `json:"stopwatch"`
	Cities    []City       `json:"cities"`
	Alerts    []Alert      `json:"alerts"`
	Defaults  AlarmOptions `json:"defaults"`
	Sounds    []Sound      `json:"sounds"`
}
type Command struct {
	Action   string        `json:"action"`
	ID       string        `json:"id"`
	Name     string        `json:"name"`
	Duration int64         `json:"duration"`
	Hour     int           `json:"hour"`
	Minute   int           `json:"minute"`
	Days     []int         `json:"days"`
	Enabled  bool          `json:"enabled"`
	Zone     string        `json:"zone"`
	Country  string        `json:"country"`
	Options  *AlarmOptions `json:"options,omitempty"`
}

type Service struct {
	mu      sync.Mutex
	state   State
	path    string
	loc     *time.Location
	now     func() Stamp
	changed func(State)
	wake    chan struct{}
	last    Stamp
	dirty   bool
}

func Path() string {
	dir := os.Getenv("XDG_DATA_HOME")
	if dir == "" {
		dir = filepath.Join(os.Getenv("HOME"), ".local/share")
	}
	return filepath.Join(dir, "whiterose/clocks/state.json")
}

func New(path string, loc *time.Location, now func() Stamp, changed func(State)) (*Service, error) {
	if loc == nil {
		loc = time.Local
	}
	if now == nil {
		now = Now
	}
	s := &Service{path: path, loc: loc, now: now, changed: changed, wake: make(chan struct{}, 1)}
	if data, err := os.ReadFile(path); err == nil {
		if len(data) > 2<<20 || json.Unmarshal(data, &s.state) != nil {
			return nil, errors.New("Clocks data couldn't be read; the file has been left unchanged")
		}
	} else if !errors.Is(err, os.ErrNotExist) {
		return nil, err
	}
	if len(s.state.Timers) > MaxItems || len(s.state.Alarms) > MaxItems || len(s.state.Cities) > MaxItems || len(s.state.Alerts) > 200 || len(s.state.Stopwatch.Laps) > 1000 {
		return nil, errors.New("Clocks data has too many entries")
	}
	n := now()
	s.last = n
	if s.state.Defaults.Sound == "" {
		s.state.Defaults = DefaultAlarmOptions()
	}
	for _, sound := range s.state.Sounds {
		if !IsCustomSound(sound.ID) || !validName(sound.Name) {
			return nil, errors.New("Invalid saved alarm sound")
		}
	}
	if len(s.state.Sounds) > 50 {
		return nil, errors.New("Too many saved alarm sounds")
	}
	if err := s.validOptions(s.state.Defaults); err != nil {
		return nil, err
	}
	for i := range s.state.Timers {
		t := &s.state.Timers[i]
		if t.Duration < 1 || t.Duration > MaxDuration || t.Remaining < 0 || t.Remaining > t.Duration || !validName(t.Name) || t.ID == "" {
			return nil, errors.New("Invalid saved timer")
		}
		if t.Running {
			t.endBoot = n.Boot + max(int64(0), t.Deadline-n.Wall)
		}
	}
	for i := range s.state.Alarms {
		a := &s.state.Alarms[i]
		if a.Options.Sound == "" {
			a.Options = DefaultAlarmOptions()
		}
		if err := s.validOptions(a.Options); err != nil {
			return nil, err
		}
		if !validAlarm(*a) {
			return nil, errors.New("Invalid saved alarm")
		}
		if a.Snooze > 0 {
			a.snoozeBoot = n.Boot + max(int64(0), a.Snooze-n.Wall)
		}
		// Don't ring stale daily alarms after a long time logged out.
		next := time.UnixMilli(a.Next).In(loc)
		if a.Enabled && (a.Next == 0 || a.Next < n.Wall-5*60*1000 || next.Hour() != a.Hour || next.Minute() != a.Minute) {
			a.Next = Next(*a, time.UnixMilli(n.Wall), loc).UnixMilli()
		}
	}
	for i := range s.state.Alerts {
		a := &s.state.Alerts[i]
		if a.Options.Sound == "" {
			a.Options = DefaultAlarmOptions()
		}
		if err := s.validOptions(a.Options); err != nil {
			return nil, err
		}
		if a.Elapsed < 0 {
			return nil, errors.New("Invalid saved alert duration")
		}
		savedAt := s.state.Now
		if savedAt == 0 {
			savedAt = a.Time
		}
		a.Elapsed += max(int64(0), n.Wall-savedAt)
		a.soundBoot = n.Boot
	}
	sw := &s.state.Stopwatch
	if sw.Elapsed < 0 {
		return nil, errors.New("Invalid saved stopwatch")
	}
	lastLap := int64(0)
	for _, lap := range sw.Laps {
		if lap < lastLap {
			return nil, errors.New("Invalid saved stopwatch lap")
		}
		lastLap = lap
	}
	if sw.Running {
		sw.Elapsed += max(int64(0), n.Wall-sw.Started)
		sw.Started, sw.boot = n.Wall, n.Boot
	}
	for _, city := range s.state.Cities {
		if _, err := location(city.Zone); err != nil || !validName(city.Name) {
			return nil, errors.New("Invalid saved world clock")
		}
	}
	return s, nil
}

func validName(name string) bool {
	return utf8.ValidString(name) && len(name) <= 200 && strings.TrimSpace(name) != "" && !strings.ContainsAny(name, "\n\r\x00")
}
func validAlarm(a Alarm) bool {
	if !validName(a.Name) || a.Hour < 0 || a.Hour > 23 || a.Minute < 0 || a.Minute > 59 || len(a.Days) > 7 {
		return false
	}
	seen := map[int]bool{}
	for _, d := range a.Days {
		if d < 0 || d > 6 || seen[d] {
			return false
		}
		seen[d] = true
	}
	return true
}
func location(zone string) (*time.Location, error) {
	if zone == "" || zone == "Local" || len(zone) > 100 || strings.Contains(zone, "..") {
		return nil, errors.New("Choose a valid IANA timezone")
	}
	return time.LoadLocation(zone)
}
func id() string {
	var b [12]byte
	_, _ = rand.Read(b[:])
	return hex.EncodeToString(b[:])
}

// Next skips nonexistent local times at the spring DST transition and uses
// one occurrence on the fall transition, rather than ringing twice.
func Next(a Alarm, after time.Time, loc *time.Location) time.Time {
	local := after.In(loc)
	for day := 0; day < 9; day++ {
		date := local.AddDate(0, 0, day)
		when := time.Date(date.Year(), date.Month(), date.Day(), a.Hour, a.Minute, 0, 0, loc)
		if when.Hour() != a.Hour || when.Minute() != a.Minute || !when.After(after) {
			continue
		}
		weekday := (int(when.Weekday()) + 6) % 7
		if len(a.Days) == 0 {
			return when
		}
		for _, d := range a.Days {
			if weekday == d {
				return when
			}
		}
	}
	return time.Time{}
}

func (s *Service) snapshot(n Stamp) State {
	// Deep copy: callers and broadcast workers never share mutable slices.
	data, _ := json.Marshal(s.state)
	var out State
	_ = json.Unmarshal(data, &out)
	out.Now = n.Wall
	out.Sounds = append(BuiltinSounds(), out.Sounds...)
	for i := range out.Alerts {
		out.Alerts[i].Elapsed = alertElapsed(s.state.Alerts[i], n)
	}
	if out.Timers == nil {
		out.Timers = []Timer{}
	}
	if out.Alarms == nil {
		out.Alarms = []Alarm{}
	}
	if out.Alerts == nil {
		out.Alerts = []Alert{}
	}
	if out.Stopwatch.Laps == nil {
		out.Stopwatch.Laps = []int64{}
	}
	for i := range out.Timers {
		if out.Timers[i].Running {
			out.Timers[i].Remaining = max(int64(0), s.state.Timers[i].endBoot-n.Boot)
			out.Timers[i].Deadline = n.Wall + out.Timers[i].Remaining
		}
	}
	for i := range out.Alarms {
		if out.Alarms[i].Snooze > 0 {
			out.Alarms[i].Snooze = n.Wall + max(int64(0), s.state.Alarms[i].snoozeBoot-n.Boot)
		}
	}
	if out.Stopwatch.Running {
		out.Stopwatch.Elapsed += max(int64(0), n.Boot-s.state.Stopwatch.boot)
		out.Stopwatch.Started = n.Wall
	}
	_, localOffset := time.UnixMilli(n.Wall).In(s.loc).Zone()
	localName := strings.ReplaceAll(filepath.Base(s.loc.String()), "_", " ")
	if localName == "Local" {
		localName = "Local time"
	}
	out.Cities = append([]City{{Name: localName, Zone: s.loc.String(), Local: true, Offset: localOffset}}, out.Cities...)
	for i := 1; i < len(out.Cities); i++ {
		if l, err := location(out.Cities[i].Zone); err == nil {
			_, out.Cities[i].Offset = time.UnixMilli(n.Wall).In(l).Zone()
		}
	}
	return out
}
func (s *Service) Snapshot() State {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.snapshot(s.now())
}

func (s *Service) save(n Stamp) error {
	if s.path == "" {
		return nil
	}
	// Save a normalized clock basis, without the derived local city.
	data, _ := json.Marshal(s.state)
	var saved State
	_ = json.Unmarshal(data, &saved)
	saved.Now = n.Wall
	for i := range saved.Alerts {
		saved.Alerts[i].Elapsed = alertElapsed(s.state.Alerts[i], n)
	}
	for i := range saved.Timers {
		if saved.Timers[i].Running {
			saved.Timers[i].Remaining = max(int64(0), s.state.Timers[i].endBoot-n.Boot)
			saved.Timers[i].Deadline = n.Wall + saved.Timers[i].Remaining
		}
	}
	for i := range saved.Alarms {
		if saved.Alarms[i].Snooze > 0 {
			saved.Alarms[i].Snooze = n.Wall + max(int64(0), s.state.Alarms[i].snoozeBoot-n.Boot)
		}
	}
	if saved.Stopwatch.Running {
		saved.Stopwatch.Elapsed += max(int64(0), n.Boot-s.state.Stopwatch.boot)
		saved.Stopwatch.Started = n.Wall
	}
	data, _ = json.Marshal(saved)
	if err := os.MkdirAll(filepath.Dir(s.path), 0o700); err != nil {
		return err
	}
	f, err := os.CreateTemp(filepath.Dir(s.path), ".clocks-*")
	if err != nil {
		return err
	}
	tmp := f.Name()
	defer os.Remove(tmp)
	if _, err = f.Write(data); err == nil {
		err = f.Sync()
	}
	closeErr := f.Close()
	if err == nil {
		err = closeErr
	}
	if err != nil {
		return err
	}
	return os.Rename(tmp, s.path)
}
func (s *Service) publish(n Stamp) State {
	s.state.Revision++
	state := s.snapshot(n)
	if s.changed != nil {
		s.changed(state)
	}
	select {
	case s.wake <- struct{}{}:
	default:
	}
	return state
}
func (s *Service) clearAlerts(item string) {
	out := s.state.Alerts[:0]
	for _, a := range s.state.Alerts {
		if a.ItemID != item {
			out = append(out, a)
		}
	}
	s.state.Alerts = out
}

func (s *Service) Apply(c Command) (State, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	n := s.now()
	before, _ := json.Marshal(s.state)
	// Runtime bases are not serialized: keep those on failed writes as well.
	backup := s.state
	backup.Timers = append([]Timer(nil), s.state.Timers...)
	backup.Alarms = append([]Alarm(nil), s.state.Alarms...)
	backup.Alerts = append([]Alert(nil), s.state.Alerts...)
	backup.Cities = append([]City(nil), s.state.Cities...)
	backup.Stopwatch.Laps = append([]int64(nil), s.state.Stopwatch.Laps...)
	backup.Sounds = append([]Sound(nil), s.state.Sounds...)
	if err := s.apply(c, n); err != nil {
		s.state = backup
		return State{}, err
	}
	after, _ := json.Marshal(s.state)
	if string(before) != string(after) {
		if err := s.save(n); err != nil {
			s.state = backup
			return State{}, fmt.Errorf("Couldn't save Clocks: %w", err)
		}
	}
	return s.publish(n), nil
}
func (s *Service) apply(c Command, n Stamp) error {
	switch c.Action {
	case "sound_register":
		if !IsCustomSound(c.ID) || !validName(c.Name) {
			return errors.New("Invalid imported sound")
		}
		for _, sound := range s.state.Sounds {
			if sound.ID == c.ID {
				return nil
			}
		}
		if len(s.state.Sounds) >= 50 {
			return errors.New("At most 50 imported alarm sounds")
		}
		s.state.Sounds = append(s.state.Sounds, Sound{ID: c.ID, Name: c.Name, Custom: true})
		return nil
	case "defaults_save":
		if c.Options == nil {
			return errors.New("Choose default alarm settings")
		}
		if err := s.validOptions(*c.Options); err != nil {
			return err
		}
		s.state.Defaults = *c.Options
		return nil
	case "timer_add":
		if c.Duration < 1 || c.Duration > MaxDuration || !validName(c.Name) || len(s.state.Timers) >= MaxItems {
			return errors.New("Name the timer and choose a duration between 1 ms and 7 days (at most 100 timers)")
		}
		s.state.Timers = append(s.state.Timers, Timer{ID: id(), Name: strings.TrimSpace(c.Name), Duration: c.Duration, Remaining: c.Duration, Running: true, Deadline: n.Wall + c.Duration, endBoot: n.Boot + c.Duration})
		return nil
	case "timer_pause", "timer_resume", "timer_reset", "timer_delete", "timer_restart":
		for i := range s.state.Timers {
			t := &s.state.Timers[i]
			if t.ID != c.ID {
				continue
			}
			switch c.Action {
			case "timer_pause":
				if t.Running {
					t.Remaining = max(int64(0), t.endBoot-n.Boot)
					t.Running = false
				}
			case "timer_resume":
				if !t.Running && t.Remaining > 0 {
					t.Running = true
					t.endBoot = n.Boot + t.Remaining
					t.Deadline = n.Wall + t.Remaining
				}
			case "timer_reset", "timer_restart":
				t.Remaining = t.Duration
				t.Running = c.Action == "timer_restart"
				t.endBoot = n.Boot + t.Duration
				t.Deadline = n.Wall + t.Duration
			case "timer_delete":
				s.state.Timers = append(s.state.Timers[:i], s.state.Timers[i+1:]...)
			}
			s.clearAlerts(c.ID)
			return nil
		}
		return errors.New("Timer no longer exists")
	case "alarm_save":
		a := Alarm{ID: c.ID, Name: strings.TrimSpace(c.Name), Hour: c.Hour, Minute: c.Minute, Days: append([]int(nil), c.Days...), Enabled: c.Enabled}
		a.Options = s.state.Defaults
		if c.ID != "" {
			for _, existing := range s.state.Alarms {
				if existing.ID == c.ID {
					a.Options = existing.Options
					break
				}
			}
		}
		if c.Options != nil {
			a.Options = *c.Options
		}
		if err := s.validOptions(a.Options); err != nil {
			return err
		}
		if !validAlarm(a) {
			return errors.New("Choose a valid time, name and repeat days")
		}
		sort.Ints(a.Days)
		if a.Enabled {
			a.Next = Next(a, time.UnixMilli(n.Wall), s.loc).UnixMilli()
		}
		if c.ID == "" {
			if len(s.state.Alarms) >= MaxItems {
				return errors.New("At most 100 alarms")
			}
			a.ID = id()
			s.state.Alarms = append(s.state.Alarms, a)
			return nil
		}
		for i := range s.state.Alarms {
			if s.state.Alarms[i].ID == c.ID {
				s.state.Alarms[i] = a
				s.clearAlerts(c.ID)
				return nil
			}
		}
		return errors.New("Alarm no longer exists")
	case "alarm_enable", "alarm_delete":
		for i := range s.state.Alarms {
			a := &s.state.Alarms[i]
			if a.ID != c.ID {
				continue
			}
			if c.Action == "alarm_delete" {
				s.state.Alarms = append(s.state.Alarms[:i], s.state.Alarms[i+1:]...)
			} else {
				a.Enabled = c.Enabled
				a.Snooze = 0
				if a.Enabled {
					a.Next = Next(*a, time.UnixMilli(n.Wall), s.loc).UnixMilli()
				} else {
					a.Next = 0
				}
			}
			s.clearAlerts(c.ID)
			return nil
		}
		return errors.New("Alarm no longer exists")
	case "stopwatch_start":
		if !s.state.Stopwatch.Running {
			s.state.Stopwatch.Running = true
			s.state.Stopwatch.Started = n.Wall
			s.state.Stopwatch.boot = n.Boot
		}
		return nil
	case "stopwatch_pause":
		sw := &s.state.Stopwatch
		if sw.Running {
			sw.Elapsed += max(int64(0), n.Boot-sw.boot)
			sw.Running = false
		}
		return nil
	case "stopwatch_reset":
		s.state.Stopwatch = Stopwatch{}
		return nil
	case "stopwatch_lap":
		sw := &s.state.Stopwatch
		if !sw.Running {
			return errors.New("Start the stopwatch before recording a lap")
		}
		if len(sw.Laps) >= 1000 {
			return errors.New("At most 1000 laps; reset the stopwatch to start again")
		}
		sw.Laps = append(sw.Laps, sw.Elapsed+max(int64(0), n.Boot-sw.boot))
		return nil
	case "city_add":
		if _, err := location(c.Zone); err != nil {
			return err
		}
		if !validName(c.Name) || len(c.Country) > 100 || len(s.state.Cities) >= MaxItems {
			return errors.New("Choose a city (at most 100 world clocks)")
		}
		for _, city := range s.state.Cities {
			if city.Zone == c.Zone {
				return errors.New("That timezone is already in your world clocks")
			}
		}
		s.state.Cities = append(s.state.Cities, City{Name: c.Name, Zone: c.Zone, Country: c.Country})
		return nil
	case "city_delete":
		for i, city := range s.state.Cities {
			if city.Zone == c.Zone {
				s.state.Cities = append(s.state.Cities[:i], s.state.Cities[i+1:]...)
				return nil
			}
		}
		return errors.New("World clock no longer exists")
	case "alert_dismiss", "alert_snooze", "alert_restart":
		for i, alert := range s.state.Alerts {
			if alert.ID != c.ID {
				continue
			}
			if c.Action == "alert_restart" {
				if alert.Kind != "timer" {
					return errors.New("Only a timer can restart")
				}
				return s.apply(Command{Action: "timer_restart", ID: alert.ItemID}, n)
			}
			if c.Action == "alert_snooze" {
				if alert.Kind != "alarm" {
					return errors.New("Only an alarm can snooze")
				}
				found := false
				for j := range s.state.Alarms {
					if s.state.Alarms[j].ID == alert.ItemID {
						s.state.Alarms[j].Snooze = n.Wall + alert.Options.SnoozeDuration
						s.state.Alarms[j].snoozeBoot = n.Boot + alert.Options.SnoozeDuration
						found = true
						break
					}
				}
				if !found {
					return errors.New("Alarm no longer exists")
				}
			}
			s.state.Alerts = append(s.state.Alerts[:i], s.state.Alerts[i+1:]...)
			return nil
		}
		return errors.New("Alert is already dismissed")
	}
	return errors.New("Unknown Clocks action")
}

// SetLocation follows the system timezone without restarting the daemon.
// Timers, snoozes and the stopwatch keep their elapsed-time basis.
func (s *Service) SetLocation(loc *time.Location) error {
	if loc == nil {
		return errors.New("Timezone is unavailable")
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	n := s.now()
	s.loc = loc
	for i := range s.state.Alarms {
		a := &s.state.Alarms[i]
		if a.Enabled {
			a.Next = Next(*a, time.UnixMilli(n.Wall), loc).UnixMilli()
		}
	}
	err := s.save(n)
	s.dirty = err != nil
	s.publish(n)
	return err
}

// Tick writes and broadcasts only on due events or changed clock bases.
// Clients animate from the last snapshot instead of receiving every tick.
func (s *Service) Tick() error {
	s.mu.Lock()
	defer s.mu.Unlock()
	n := s.now()
	wallDelta := (n.Wall - s.last.Wall) - (n.Boot - s.last.Boot)
	changed := wallDelta > 1000 || wallDelta < -1000
	s.last = n
	for i := range s.state.Timers {
		t := &s.state.Timers[i]
		if t.Running && t.endBoot <= n.Boot {
			t.Running, t.Remaining = false, 0
			s.state.Alerts = append(s.state.Alerts, Alert{ID: id(), Kind: "timer", ItemID: t.ID, Name: t.Name, Time: n.Wall, Message: "Your timer is complete.", Options: DefaultAlarmOptions(), soundBoot: n.Boot})
			changed = true
		}
	}
	for i := range s.state.Alarms {
		a := &s.state.Alarms[i]
		if a.Snooze > 0 && a.snoozeBoot <= n.Boot {
			s.state.Alerts = append(s.state.Alerts, s.newAlarmAlert(*a, n, "Snoozed alarm"))
			a.Snooze = 0
			changed = true
		}
		if a.Enabled && a.Next > 0 && a.Next <= n.Wall {
			when := time.UnixMilli(a.Next)
			s.state.Alerts = append(s.state.Alerts, s.newAlarmAlert(*a, n, when.In(s.loc).Format("15:04 · Monday")))
			if len(a.Days) == 0 {
				a.Enabled, a.Next = false, 0
			} else {
				a.Next = Next(*a, time.UnixMilli(n.Wall), s.loc).UnixMilli()
			}
			changed = true
		}
	}
	for i := range s.state.Alerts {
		a := &s.state.Alerts[i]
		if !a.Silenced && a.Options.RingDuration > 0 && alertElapsed(*a, n) >= a.Options.RingDuration {
			a.Silenced = true
			changed = true
		}
	}
	if changed || s.dirty {
		if len(s.state.Alerts) > 200 {
			s.state.Alerts = s.state.Alerts[len(s.state.Alerts)-200:]
		}
		// Due alerts are still delivered if the disk became unwritable; return
		// the save error for diagnostics, but never suppress an alarm.
		err := s.save(n)
		s.dirty = err != nil
		if changed {
			s.publish(n)
		}
		return err
	}
	return nil
}

func (s *Service) Wait() time.Duration {
	s.mu.Lock()
	defer s.mu.Unlock()
	n := s.now()
	wait := time.Duration(0)
	add := func(ms int64) {
		d := time.Duration(max(int64(1), ms)) * time.Millisecond
		if wait == 0 || d < wait {
			wait = d
		}
	}
	for _, t := range s.state.Timers {
		if t.Running {
			add(t.endBoot - n.Boot)
		}
	}
	for _, a := range s.state.Alarms {
		if a.Enabled {
			add(a.Next - n.Wall)
		}
		if a.Snooze > 0 {
			add(a.snoozeBoot - n.Boot)
		}
	}
	for _, a := range s.state.Alerts {
		if !a.Silenced && a.Options.RingDuration > 0 {
			add(a.Options.RingDuration - alertElapsed(a, n))
		}
	}
	if s.state.Stopwatch.Running || s.dirty {
		add(1000)
	}
	// Recheck wall-clock and resume changes at most a second late. Completely
	// idle Clocks has no timer and sleeps until a command arrives.
	if wait > time.Second {
		wait = time.Second
	}
	return wait
}

func (s *Service) Run(done <-chan struct{}, report func(error)) {
	for {
		if err := s.Tick(); err != nil && report != nil {
			report(err)
		}
		var timer *time.Timer
		var tick <-chan time.Time
		if d := s.Wait(); d > 0 {
			timer = time.NewTimer(d)
			tick = timer.C
		}
		select {
		case <-done:
			if timer != nil {
				timer.Stop()
			}
			return
		case <-s.wake:
		case <-tick:
		}
		if timer != nil {
			timer.Stop()
		}
	}
}
