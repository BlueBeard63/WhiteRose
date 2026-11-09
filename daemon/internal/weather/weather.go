// Package weather gets the current conditions for the calendar panel's
// weather pill from Open-Meteo (free, no account or API key). The place is
// set by the user in ~/.config/whiterose/weather.toml; nothing is detected:
//
//	city = "London"          # looked up once, then cached
//	# latitude = 51.5        # or give coordinates directly
//	# longitude = -0.12
//	# units = "fahrenheit"   # default celsius
package weather

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"math"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strconv"
	"time"

	"github.com/BurntSushi/toml"
)

// Config is weather.toml.
type Config struct {
	City      string   `toml:"city"`
	Latitude  *float64 `toml:"latitude"`
	Longitude *float64 `toml:"longitude"`
	Units     string   `toml:"units"`
}

// Now is what the pill shows (also the cached file).
type Now struct {
	Temperature int       `json:"temperature"`
	Unit        string    `json:"unit"` // "C" or "F"
	Code        int       `json:"code"` // WMO weather code
	Day         bool      `json:"day"`
	Icon        string    `json:"icon"`
	Description string    `json:"description"`
	Place       string    `json:"place"`
	Updated     time.Time `json:"updated"`
	// The looked-up coordinates, so the city is geocoded only once.
	City      string  `json:"city,omitempty"`
	Latitude  float64 `json:"latitude"`
	Longitude float64 `json:"longitude"`
}

// API endpoints (replaced in tests).
var (
	GeocodeURL  = "https://geocoding-api.open-meteo.com/v1/search"
	ForecastURL = "https://api.open-meteo.com/v1/forecast"
)

// ErrNotConfigured: no weather.toml (or no place in it): the pill stays hidden.
var ErrNotConfigured = errors.New("no place set in weather.toml")

func LoadConfig(configDir string) (Config, error) {
	var c Config
	_, err := toml.DecodeFile(filepath.Join(configDir, "weather.toml"), &c)
	if errors.Is(err, os.ErrNotExist) || err == nil && c.City == "" && (c.Latitude == nil || c.Longitude == nil) {
		return c, ErrNotConfigured
	}
	if err != nil {
		return c, fmt.Errorf("weather.toml: %w", err)
	}
	return c, nil
}

// Cached reads the last reading, if any.
func Cached(cacheDir string) (Now, bool) {
	var n Now
	b, err := os.ReadFile(filepath.Join(cacheDir, "weather.json"))
	if err != nil || json.Unmarshal(b, &n) != nil {
		return n, false
	}
	return n, true
}

// Refresh fetches the current conditions and caches them.
func Refresh(ctx context.Context, configDir, cacheDir string, client *http.Client) (Now, error) {
	cfg, err := LoadConfig(configDir)
	if err != nil {
		return Now{}, err
	}
	prev, _ := Cached(cacheDir)
	n := Now{Place: cfg.City, City: cfg.City}
	switch {
	case cfg.Latitude != nil && cfg.Longitude != nil:
		n.Latitude, n.Longitude = *cfg.Latitude, *cfg.Longitude
	case prev.City == cfg.City && (prev.Latitude != 0 || prev.Longitude != 0):
		n.Latitude, n.Longitude, n.Place = prev.Latitude, prev.Longitude, prev.Place
	default:
		if n.Latitude, n.Longitude, n.Place, err = geocode(ctx, client, cfg.City); err != nil {
			return Now{}, err
		}
	}
	unit := "celsius"
	n.Unit = "C"
	if cfg.Units == "fahrenheit" {
		unit, n.Unit = "fahrenheit", "F"
	}
	q := url.Values{
		"latitude":         {strconv.FormatFloat(n.Latitude, 'f', 4, 64)},
		"longitude":        {strconv.FormatFloat(n.Longitude, 'f', 4, 64)},
		"current":          {"temperature_2m,weather_code,is_day"},
		"temperature_unit": {unit},
	}
	var resp struct {
		Current struct {
			Temperature float64 `json:"temperature_2m"`
			Code        int     `json:"weather_code"`
			IsDay       int     `json:"is_day"`
		} `json:"current"`
	}
	if err := getJSON(ctx, client, ForecastURL+"?"+q.Encode(), &resp); err != nil {
		return Now{}, err
	}
	n.Temperature = int(math.Round(resp.Current.Temperature))
	n.Code, n.Day = resp.Current.Code, resp.Current.IsDay == 1
	n.Icon, n.Description = Describe(n.Code, n.Day)
	n.Updated = time.Now()
	b, _ := json.Marshal(n)
	if err := os.MkdirAll(cacheDir, 0o700); err != nil {
		return n, err
	}
	tmp := filepath.Join(cacheDir, ".weather.json.tmp")
	if err := os.WriteFile(tmp, b, 0o600); err != nil {
		return n, err
	}
	return n, os.Rename(tmp, filepath.Join(cacheDir, "weather.json"))
}

func geocode(ctx context.Context, client *http.Client, city string) (lat, lon float64, name string, err error) {
	var resp struct {
		Results []struct {
			Name      string  `json:"name"`
			Latitude  float64 `json:"latitude"`
			Longitude float64 `json:"longitude"`
		} `json:"results"`
	}
	q := url.Values{"name": {city}, "count": {"1"}, "format": {"json"}}
	if err := getJSON(ctx, client, GeocodeURL+"?"+q.Encode(), &resp); err != nil {
		return 0, 0, "", err
	}
	if len(resp.Results) == 0 {
		return 0, 0, "", fmt.Errorf("no place called %q", city)
	}
	r := resp.Results[0]
	return r.Latitude, r.Longitude, r.Name, nil
}

func getJSON(ctx context.Context, client *http.Client, u string, into any) error {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, u, nil)
	if err != nil {
		return err
	}
	resp, err := client.Do(req)
	if err != nil {
		return errors.New("couldn't reach the weather service")
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("weather service answered %s", resp.Status)
	}
	return json.NewDecoder(io.LimitReader(resp.Body, 1<<20)).Decode(into)
}

// Describe maps a WMO weather code to the shell's icon name and words.
func Describe(code int, day bool) (icon, description string) {
	pick := func(d, n string) string {
		if day {
			return d
		}
		return n
	}
	switch {
	case code == 0:
		return pick("sun", "moon"), "Clear"
	case code == 1 || code == 2:
		return pick("cloudSun", "cloudMoon"), "Partly cloudy"
	case code == 3:
		return "cloud", "Overcast"
	case code == 45 || code == 48:
		return "cloudFog", "Fog"
	case code >= 51 && code <= 57:
		return "cloudDrizzle", "Drizzle"
	case code >= 61 && code <= 67, code >= 80 && code <= 82:
		return "cloudRain", "Rain"
	case code >= 71 && code <= 77, code == 85 || code == 86:
		return "cloudSnow", "Snow"
	case code >= 95 && code <= 99:
		return "cloudLightning", "Thunderstorm"
	}
	return "cloud", "Cloudy"
}
