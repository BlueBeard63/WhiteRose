package weather

import (
	"context"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func fakeAPI(t *testing.T) (*httptest.Server, *int) {
	geocodes := 0
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch {
		case strings.HasPrefix(r.URL.Path, "/geo"):
			geocodes++
			if r.URL.Query().Get("name") == "Nowhere" {
				_, _ = w.Write([]byte(`{}`))
				return
			}
			_, _ = w.Write([]byte(`{"results":[{"name":"London","latitude":51.5085,"longitude":-0.1257}]}`))
		case strings.HasPrefix(r.URL.Path, "/forecast"):
			if r.URL.Query().Get("latitude") != "51.5085" {
				t.Errorf("forecast for the wrong place: %s", r.URL.RawQuery)
			}
			temp := "11.6"
			if r.URL.Query().Get("temperature_unit") == "fahrenheit" {
				temp = "52.9"
			}
			_, _ = w.Write([]byte(`{"current":{"temperature_2m":` + temp + `,"weather_code":2,"is_day":1}}`))
		}
	}))
	GeocodeURL, ForecastURL = srv.URL+"/geo", srv.URL+"/forecast"
	t.Cleanup(srv.Close)
	return srv, &geocodes
}

func TestCityIsLookedUpOnceThenCached(t *testing.T) {
	srv, geocodes := fakeAPI(t)
	cfg, cache := t.TempDir(), t.TempDir()
	if _, err := Refresh(context.Background(), cfg, cache, srv.Client()); err != ErrNotConfigured {
		t.Fatalf("no weather.toml: %v", err)
	}
	_ = os.WriteFile(filepath.Join(cfg, "weather.toml"), []byte(`city = "London"`), 0o644)
	n, err := Refresh(context.Background(), cfg, cache, srv.Client())
	if err != nil || n.Temperature != 12 || n.Unit != "C" || n.Icon != "cloudSun" || n.Place != "London" {
		t.Fatalf("got %+v %v", n, err)
	}
	if _, err := Refresh(context.Background(), cfg, cache, srv.Client()); err != nil || *geocodes != 1 {
		t.Fatalf("second refresh geocoded again (%d) or failed: %v", *geocodes, err)
	}
	if c, ok := Cached(cache); !ok || c.Temperature != 12 {
		t.Fatalf("cache: %+v %v", c, ok)
	}
	_ = os.WriteFile(filepath.Join(cfg, "weather.toml"), []byte("city = \"London\"\nunits = \"fahrenheit\""), 0o644)
	if n, _ := Refresh(context.Background(), cfg, cache, srv.Client()); n.Temperature != 53 || n.Unit != "F" {
		t.Fatalf("fahrenheit: %+v", n)
	}
}

func TestUnknownCityAndCoordinates(t *testing.T) {
	srv, geocodes := fakeAPI(t)
	cfg, cache := t.TempDir(), t.TempDir()
	_ = os.WriteFile(filepath.Join(cfg, "weather.toml"), []byte(`city = "Nowhere"`), 0o644)
	if _, err := Refresh(context.Background(), cfg, cache, srv.Client()); err == nil || !strings.Contains(err.Error(), "Nowhere") {
		t.Fatalf("unknown city: %v", err)
	}
	_ = os.WriteFile(filepath.Join(cfg, "weather.toml"), []byte("latitude = 51.5085\nlongitude = -0.1257"), 0o644)
	before := *geocodes
	if _, err := Refresh(context.Background(), cfg, cache, srv.Client()); err != nil || *geocodes != before {
		t.Fatalf("coordinates must skip the lookup: %v", err)
	}
}

func TestDescribe(t *testing.T) {
	for _, c := range []struct {
		code      int
		day       bool
		icon, say string
	}{
		{0, true, "sun", "Clear"}, {0, false, "moon", "Clear"}, {2, false, "cloudMoon", "Partly cloudy"},
		{3, true, "cloud", "Overcast"}, {45, true, "cloudFog", "Fog"}, {53, true, "cloudDrizzle", "Drizzle"},
		{63, true, "cloudRain", "Rain"}, {81, true, "cloudRain", "Rain"}, {73, true, "cloudSnow", "Snow"},
		{95, true, "cloudLightning", "Thunderstorm"}, {42, true, "cloud", "Cloudy"},
	} {
		if icon, say := Describe(c.code, c.day); icon != c.icon || say != c.say {
			t.Errorf("code %d day %v: %s %s", c.code, c.day, icon, say)
		}
	}
}
