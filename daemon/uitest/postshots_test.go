//go:build uitest && shots

// Full-desktop screenshots for the portfolio write-up (not checks): the
// desktop, the launcher, the overview, quick settings and Settings, all with
// a real wallpaper and the dark/rose theme. Gated behind WHITEROSE_POSTSHOTS
// so `make shots` (which runs TestShots) never triggers it. Run with:
//
//	cd daemon && WHITEROSE_POSTSHOTS=/abs/out go test -tags 'uitest shots' \
//	  -count=1 -timeout 15m -run TestPostShots ./uitest/
//
// WHITEROSE_WALLPAPER sets the wallpaper image (an absolute path).
package uitest_test

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	"whiterose/daemon/internal/uitest"
)

func TestPostShots(t *testing.T) {
	out := os.Getenv("WHITEROSE_POSTSHOTS")
	if out == "" {
		t.Skip("WHITEROSE_POSTSHOTS isn't set")
	}
	wallpaper := os.Getenv("WHITEROSE_WALLPAPER")

	// Everything shares the dark theme, the rose accent and the real wallpaper.
	look := func(extra map[string]any) map[string]any {
		s := map[string]any{
			"appearance.theme":  "dark",
			"appearance.accent": "#E8A3AE",
		}
		if wallpaper != "" {
			s["wallpaper.current"] = wallpaper
		}
		for k, v := range extra {
			s[k] = v
		}
		return s
	}
	screenOf := func(s *uitest.Session) [4]int { return s.Layer("whiterose-wallpaper").Geometry() }
	full := func(s *uitest.Session, name string) {
		g := screenOf(s)
		save(t, filepath.Join(out, name+".png"), s.Capture(0, 0, g[2], g[3]))
	}

	// 1. The desktop at rest: wallpaper, the bar and the idle island.
	t.Run("desktop", func(t *testing.T) {
		s := uitest.Start(t, uitest.Options{Shell: true, Settings: look(nil)})
		s.Bar()
		time.Sleep(1200 * time.Millisecond)
		full(s, "desktop")
	})

	// 2. The launcher dropping from the island (SUPER).
	t.Run("launcher", func(t *testing.T) {
		s := uitest.Start(t, uitest.Options{Shell: true, Settings: look(nil)})
		bar := s.Bar()
		bar.Press("super")
		bar.WaitFor(uitest.Named("Search").Role("text"))
		time.Sleep(800 * time.Millisecond)
		full(s, "launcher")
	})

	// 3. The overview with a few windows open (SUPER+W): the month's headline.
	t.Run("overview", func(t *testing.T) {
		s := uitest.Start(t, uitest.Options{Shell: true, Settings: look(nil)})
		for _, name := range []string{"Documents", "Pictures", "Downloads"} {
			dir := filepath.Join(s.Home, name)
			os.MkdirAll(dir, 0o755)
			s.Open(filesBinary(s)+" "+dir, name+" - Files")
		}
		time.Sleep(600 * time.Millisecond)
		s.Bar().Press("super+w")
		s.Layer("whiterose-overview")
		time.Sleep(1200 * time.Millisecond)
		full(s, "overview")
	})

	// 4. Quick settings open from the bar.
	t.Run("quick-settings", func(t *testing.T) {
		s := uitest.Start(t, uitest.Options{Shell: true, Settings: look(nil)})
		bar := s.Bar()
		bar.Click(uitest.Named("Quick settings").Role("push button"))
		time.Sleep(900 * time.Millisecond)
		full(s, "quick-settings")
	})

	// 5. Settings on Appearance: the wallpapers, theme and accent.
	t.Run("settings-appearance", func(t *testing.T) {
		s := uitest.Start(t, uitest.Options{Shell: true, Settings: look(nil)})
		settings := s.Settings("appearance")
		settings.WaitFor(uitest.Named("Appearance").Role("page tab"))
		time.Sleep(900 * time.Millisecond)
		g := settings.Geometry()
		save(t, filepath.Join(out, "settings-appearance.png"), s.Capture(g[0], g[1], g[2], g[3]))
	})
}
