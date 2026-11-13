// Package migrate imports the configuration files WhiteRose used before the
// settings database into it, once: each file that imports cleanly is renamed
// to <name>.migrated (kept, never deleted). A file that can't be read is left
// where it is and reported, so nothing is lost. Dropping one of these files
// back in imports it again on the next start.
package migrate

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"

	"github.com/BurntSushi/toml"

	"whiterose/daemon/internal/config"
	"whiterose/daemon/internal/store"
)

// Dirs are the old files' homes.
type Dirs struct {
	Config string // ~/.config/whiterose
	State  string // ~/.local/state/whiterose
}

func DefaultDirs() Dirs {
	home, _ := os.UserHomeDir()
	env := func(k, def string) string {
		if v := os.Getenv(k); v != "" {
			return v
		}
		return filepath.Join(home, def)
	}
	return Dirs{
		Config: filepath.Join(env("XDG_CONFIG_HOME", ".config"), "whiterose"),
		State:  filepath.Join(env("XDG_STATE_HOME", ".local/state"), "whiterose"),
	}
}

// Report says what happened to each file found.
type Report struct {
	Imported []string
	Failed   map[string]error
}

type importer struct {
	file string
	run  func(data []byte) error
}

// Run imports every old file present.
func Run(st *store.Store, secrets *store.Resolver, d Dirs) Report {
	set := func(key string, v any) error { return st.SetValue(key, v) }
	importers := []importer{
		{filepath.Join(d.Config, "dock.json"), func(b []byte) error {
			var f struct {
				Pinned []string `json:"pinned"`
			}
			if err := json.Unmarshal(b, &f); err != nil {
				return err
			}
			return set("dock.pinned", f.Pinned)
		}},
		{filepath.Join(d.Config, "launcher.json"), func(b []byte) error {
			var f struct {
				Launches     map[string]int      `json:"launches"`
				Commands     []string            `json:"commands"`
				Calculations []store.Calculation `json:"calculations"`
			}
			if err := json.Unmarshal(b, &f); err != nil {
				return err
			}
			if f.Launches != nil {
				if err := set("launcher.launches", f.Launches); err != nil {
					return err
				}
			}
			if f.Commands != nil {
				if err := set("launcher.commands", f.Commands); err != nil {
					return err
				}
			}
			if f.Calculations != nil {
				return set("launcher.calculations", f.Calculations)
			}
			return nil
		}},
		{filepath.Join(d.State, "quick.json"), func(b []byte) error {
			var f struct {
				DoNotDisturb          *bool    `json:"doNotDisturb"`
				NightLightUntil       *float64 `json:"nightLightUntil"`
				NightLightTemperature *int     `json:"nightLightTemperature"`
			}
			if err := json.Unmarshal(b, &f); err != nil {
				return err
			}
			if f.DoNotDisturb != nil {
				if err := set("quick.doNotDisturb", *f.DoNotDisturb); err != nil {
					return err
				}
			}
			if f.NightLightUntil != nil {
				if err := set("quick.nightLight.until", *f.NightLightUntil); err != nil {
					return err
				}
			}
			if f.NightLightTemperature != nil {
				return set("quick.nightLight.temperature", *f.NightLightTemperature)
			}
			return nil
		}},
		{filepath.Join(d.Config, "weather.toml"), func(b []byte) error {
			var f struct {
				City      string   `toml:"city"`
				Latitude  *float64 `toml:"latitude"`
				Longitude *float64 `toml:"longitude"`
				Units     string   `toml:"units"`
			}
			if _, err := toml.NewDecoder(bytes.NewReader(b)).Decode(&f); err != nil {
				return err
			}
			if err := set("weather.city", f.City); err != nil {
				return err
			}
			if err := set("weather.latitude", f.Latitude); err != nil {
				return err
			}
			if err := set("weather.longitude", f.Longitude); err != nil {
				return err
			}
			if f.Units != "" {
				return set("weather.units", f.Units)
			}
			return nil
		}},
		{filepath.Join(d.Config, "calendars.toml"), func(b []byte) error {
			var f struct {
				Calendar []struct {
					Name  string `toml:"name"`
					URL   string `toml:"url"`
					Path  string `toml:"path"`
					Color string `toml:"color"`
				} `toml:"calendar"`
			}
			if _, err := toml.NewDecoder(bytes.NewReader(b)).Decode(&f); err != nil {
				return err
			}
			var existing []store.CalendarSource
			_ = st.GetInto("calendar.sources", &existing)
			for _, c := range f.Calendar {
				src := store.CalendarSource{Name: c.Name, Color: c.Color, Path: c.Path}
				if c.URL != "" {
					src.Secret = store.NewSecretID()
					if _, err := secrets.Set(src.Secret, c.URL); err != nil {
						return fmt.Errorf("storing %q's link: %w", c.Name, err)
					}
				}
				existing = append(existing, src)
			}
			return set("calendar.sources", existing)
		}},
		{filepath.Join(d.Config, "displays.toml"), func(b []byte) error {
			disp, err := config.ParseDisplays(bytes.NewReader(b))
			if err != nil {
				return err
			}
			return set("displays", disp)
		}},
	}

	r := Report{Failed: map[string]error{}}
	for _, im := range importers {
		b, err := os.ReadFile(im.file)
		if errors.Is(err, os.ErrNotExist) {
			continue
		}
		if err == nil {
			err = im.run(b)
		}
		if err == nil {
			err = os.Rename(im.file, im.file+".migrated")
		}
		if err != nil {
			r.Failed[im.file] = err
			continue
		}
		r.Imported = append(r.Imported, im.file)
	}
	return r
}
