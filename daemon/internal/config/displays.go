// Package config loads and validates WhiteRose configuration files.
package config

import (
	"errors"
	"fmt"
	"io"
	"math"
	"os"
	"regexp"
	"sort"
	"strconv"
	"strings"

	"github.com/BurntSushi/toml"

	"whiterose/daemon/internal/monitor"
)

// Displays is ~/.config/whiterose/displays.toml.
//
// Per-monitor preferences (mode, scale, ...) live in [[monitor]] and follow the
// monitor everywhere. Placement depends on which monitors are plugged in
// together, so it lives in [[profile]].
type Displays struct {
	Monitors []Monitor `toml:"monitor"`
	Profiles []Profile `toml:"profile"`
}

// Monitor holds preferences for one monitor, keyed by its stable ID.
type Monitor struct {
	ID        string            `toml:"id"`
	Label     string            `toml:"label"`
	Mode      string            `toml:"mode"`  // "2560x1440@165", "2560x1440", or "" / "preferred"
	Scale     float64           `toml:"scale"` // 0 means 1
	Transform monitor.Transform `toml:"transform"`
	VRR       string            `toml:"vrr"` // "", "auto", "on", "off"
}

// Profile is a layout for one exact set of connected monitors.
type Profile struct {
	Name    string          `toml:"name"`
	Outputs []ProfileOutput `toml:"output"`
}

// ProfileOutput places one monitor within a profile.
type ProfileOutput struct {
	ID       string `toml:"id"`
	Position [2]int `toml:"position"`
	// Disabled turns the monitor off while this profile is active.
	Disabled bool `toml:"disabled"`
}

// Mode is a parsed display mode. Refresh is 0 when unspecified.
type Mode struct {
	Width, Height int
	Refresh       float64
}

var modeRe = regexp.MustCompile(`^([1-9][0-9]{2,4})x([1-9][0-9]{2,4})(?:@([0-9]{1,3}(?:\.[0-9]{1,3})?))?$`)

// ParseMode parses "WxH" or "WxH@Hz". The empty string and "preferred"
// return ok=false with no error: use the monitor's preferred mode.
func ParseMode(s string) (m Mode, ok bool, err error) {
	if s == "" || s == "preferred" {
		return Mode{}, false, nil
	}
	sub := modeRe.FindStringSubmatch(s)
	if sub == nil {
		return Mode{}, false, fmt.Errorf("invalid mode %q (want e.g. 2560x1440@165)", s)
	}
	m.Width, _ = strconv.Atoi(sub[1])
	m.Height, _ = strconv.Atoi(sub[2])
	if sub[3] != "" {
		m.Refresh, _ = strconv.ParseFloat(sub[3], 64)
		if m.Refresh == 0 {
			return Mode{}, false, fmt.Errorf("invalid mode %q: refresh rate is 0", s)
		}
	}
	return m, true, nil
}

var idRe = regexp.MustCompile(`^(builtin|edid:[0-9a-f]{12}(@[A-Za-z0-9:._-]+)?|connector:[A-Za-z0-9._-]+)$`)

// ValidID reports whether s has the shape of a monitor ID.
func ValidID(s string) bool { return idRe.MatchString(s) }

// LoadDisplays reads and validates a displays.toml file.
func LoadDisplays(path string) (*Displays, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()
	d, err := ParseDisplays(f)
	if err != nil {
		return nil, fmt.Errorf("%s: %w", path, err)
	}
	return d, nil
}

// ParseDisplays decodes and validates displays config. Unknown keys are
// rejected so typos (e.g. "postion") don't silently do nothing.
func ParseDisplays(r io.Reader) (*Displays, error) {
	var d Displays
	md, err := toml.NewDecoder(r).Decode(&d)
	if err != nil {
		return nil, err
	}
	if und := md.Undecoded(); len(und) > 0 {
		keys := make([]string, len(und))
		for i, k := range und {
			keys[i] = k.String()
		}
		return nil, fmt.Errorf("unknown keys: %s", strings.Join(keys, ", "))
	}
	if err := d.Validate(); err != nil {
		return nil, err
	}
	return &d, nil
}

// Validate reports every problem found, not just the first.
func (d *Displays) Validate() error {
	var errs []error
	fail := func(format string, a ...any) { errs = append(errs, fmt.Errorf(format, a...)) }

	seenMon := map[string]int{}
	for i, m := range d.Monitors {
		at := fmt.Sprintf("monitor[%d]", i)
		switch {
		case m.ID == "":
			fail("%s.id: required", at)
		case !ValidID(m.ID):
			fail("%s.id: invalid monitor id %q", at, m.ID)
		default:
			if j, dup := seenMon[m.ID]; dup {
				fail("%s.id: %q already used by monitor[%d]", at, m.ID, j)
			}
			seenMon[m.ID] = i
		}
		if _, _, err := ParseMode(m.Mode); err != nil {
			fail("%s.mode: %v", at, err)
		}
		if err := validScale(m.Scale); err != nil {
			fail("%s.scale: %v", at, err)
		}
		if !m.Transform.Valid() {
			fail("%s.transform: invalid %q", at, m.Transform)
		}
		switch m.VRR {
		case "", "auto", "on", "off":
		default:
			fail("%s.vrr: must be auto, on or off (got %q)", at, m.VRR)
		}
	}

	seenProf := map[string]int{}
	for i, p := range d.Profiles {
		at := fmt.Sprintf("profile[%d]", i)
		if p.Name == "" {
			fail("%s.name: required", at)
		} else if j, dup := seenProf[p.Name]; dup {
			fail("%s.name: %q already used by profile[%d]", at, p.Name, j)
		} else {
			seenProf[p.Name] = i
		}
		if len(p.Outputs) == 0 {
			fail("%s: needs at least one output", at)
		}
		ids := map[string]bool{}
		enabled := 0
		for k, o := range p.Outputs {
			oat := fmt.Sprintf("%s.output[%d]", at, k)
			if !ValidID(o.ID) {
				fail("%s.id: invalid monitor id %q", oat, o.ID)
			}
			if ids[o.ID] {
				fail("%s.id: %q listed twice", oat, o.ID)
			}
			ids[o.ID] = true
			if !o.Disabled {
				enabled++
			}
		}
		if len(p.Outputs) > 0 && enabled == 0 {
			fail("%s: every output is disabled", at)
		}
	}
	return errors.Join(errs...)
}

// validScale accepts 0 (unset) or 0.5–4 in steps of 1/120, the precision of
// the Wayland fractional-scale protocol.
func validScale(s float64) error {
	if s == 0 {
		return nil
	}
	if s < 0.5 || s > 4 {
		return fmt.Errorf("must be between 0.5 and 4 (got %g)", s)
	}
	if v := s * 120; math.Abs(v-math.Round(v)) > 1e-6 {
		return fmt.Errorf("%g is not a multiple of 1/120", s)
	}
	return nil
}

// Monitor returns the preferences for id, or zero-value defaults.
func (d *Displays) Monitor(id string) Monitor {
	for _, m := range d.Monitors {
		if m.ID == id {
			return m
		}
	}
	return Monitor{ID: id}
}

// MatchProfile returns the first profile whose outputs are exactly the
// connected set. Order in the file breaks ties, so users control priority.
// ok is false when nothing matches; callers fall back to monitor.AutoLayout.
func (d *Displays) MatchProfile(connected []string) (p Profile, ok bool) {
	want := append([]string(nil), connected...)
	sort.Strings(want)
	for _, p := range d.Profiles {
		have := make([]string, len(p.Outputs))
		for i, o := range p.Outputs {
			have[i] = o.ID
		}
		sort.Strings(have)
		if equal(have, want) {
			return p, true
		}
	}
	return Profile{}, false
}

func equal(a, b []string) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}
