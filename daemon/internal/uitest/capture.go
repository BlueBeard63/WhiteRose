package uitest

import (
	"image"
	"image/png"
	"os"
	"path/filepath"
	"sort"
	"time"
)

// Capture is what the screen shows in the region x, y, w, h (logical
// pixels; the picture has the output's physical pixels), taken by the
// compositor itself, as a screenshot is: it saves the PNG in the session's
// Pictures/Screenshots, and this reads it back.
func (s *Session) Capture(x, y, w, h int) image.Image {
	s.t.Helper()
	dir := filepath.Join(s.Home, "Pictures", "Screenshots")
	before := pngs(dir)
	s.send(map[string]any{"cmd": "capture", "region": []int{x, y, w, h}, "pointer": false, "save": true})
	deadline := time.Now().Add(10 * time.Second)
	for time.Now().Before(deadline) {
		if now := pngs(dir); len(now) > len(before) {
			path := newest(now, before)
			// The PNG is written by a worker thread: wait until it decodes.
			for time.Now().Before(deadline) {
				if img, err := decodePNG(path); err == nil {
					return img
				}
				time.Sleep(50 * time.Millisecond)
			}
		}
		time.Sleep(50 * time.Millisecond)
	}
	s.t.Fatalf("no screenshot of %d,%d %dx%d in %s", x, y, w, h, dir)
	return nil
}

func pngs(dir string) []string {
	m, _ := filepath.Glob(filepath.Join(dir, "*.png"))
	sort.Strings(m)
	return m
}

// newest is the one path in `now` that isn't in `before`.
func newest(now, before []string) string {
	seen := map[string]bool{}
	for _, p := range before {
		seen[p] = true
	}
	for _, p := range now {
		if !seen[p] {
			return p
		}
	}
	return now[len(now)-1]
}

func decodePNG(path string) (image.Image, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()
	return png.Decode(f)
}
