// Package calendar collects the user's calendars for the shell's calendar
// panel: iCal links (Google, Outlook, iCloud "secret address", any .ics URL)
// and .ics files from the settings ("calendar.sources", links resolved from
// the keyring by the caller), plus .ics files dropped into
// ~/.local/share/whiterose/calendars/. Links are fetched into
// ~/.cache/whiterose/calendars so the panel works offline and opens without
// waiting for the network. Read-only.
package calendar

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"whiterose/daemon/internal/ical"
)

// Source is one calendar, with its link (if any) already resolved.
type Source struct {
	Name  string
	URL   string
	Path  string
	Color string
}

// Palette for calendars without a colour (the Figma accents).
var Palette = []string{"#E8A3AE", "#8FB8F2", "#9BD5A8", "#F2C98F", "#B9A3E8", "#F2A68F"}

// Dirs are where things live (overridable in tests).
type Dirs struct {
	Config string // ~/.config/whiterose (only for importing old config files)
	Data   string // ~/.local/share/whiterose
	Cache  string // ~/.cache/whiterose
	Home   string
}

// DefaultDirs follows the XDG base directory spec.
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
		Data:   filepath.Join(env("XDG_DATA_HOME", ".local/share"), "whiterose"),
		Cache:  filepath.Join(env("XDG_CACHE_HOME", ".cache"), "whiterose"),
		Home:   home,
	}
}

// Sources is the configured calendars plus any dropped-in .ics files, each
// with a colour.
func Sources(d Dirs, configured []Source) []Source {
	out := append([]Source(nil), configured...)
	drop, _ := filepath.Glob(filepath.Join(d.Data, "calendars", "*.ics"))
	sort.Strings(drop)
	for _, p := range drop {
		out = append(out, Source{Name: strings.TrimSuffix(filepath.Base(p), ".ics"), Path: p})
	}
	for i := range out {
		if out[i].Color == "" {
			out[i].Color = Palette[i%len(Palette)]
		}
		if strings.HasPrefix(out[i].Path, "~/") {
			out[i].Path = filepath.Join(d.Home, out[i].Path[2:])
		}
	}
	return out
}

// cachePath is where a link's last download is kept (named by a hash: the
// link itself is a secret for most providers).
func cachePath(d Dirs, url string) string {
	h := sha256.Sum256([]byte(url))
	return filepath.Join(d.Cache, "calendars", hex.EncodeToString(h[:8])+".ics")
}

// MaxFeed bounds a download (years of history fit easily).
const MaxFeed = 32 << 20

// Refresh downloads every linked calendar into the cache. A failed fetch
// keeps the previous copy. Returns how many changed; errors never include the
// link (it's a secret).
func Refresh(ctx context.Context, d Dirs, srcs []Source, client *http.Client) (changed int, errs []error) {
	for _, s := range srcs {
		if s.URL == "" {
			continue
		}
		ok, err := fetch(ctx, client, s.URL, cachePath(d, s.URL))
		if err != nil {
			errs = append(errs, fmt.Errorf("calendar %q: %w", s.Name, err))
		}
		if ok {
			changed++
		}
	}
	return changed, errs
}

func fetch(ctx context.Context, client *http.Client, url, dest string) (bool, error) {
	if after, ok := strings.CutPrefix(url, "webcal://"); ok {
		url = "https://" + after
	}
	if !strings.HasPrefix(url, "https://") && !strings.HasPrefix(url, "http://") {
		return false, errors.New("only http(s) and webcal links are supported")
	}
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return false, errors.New("bad link")
	}
	if etag, err := os.ReadFile(dest + ".etag"); err == nil {
		req.Header.Set("If-None-Match", strings.TrimSpace(string(etag)))
	}
	resp, err := client.Do(req)
	if err != nil {
		return false, errors.New("couldn't reach the server")
	}
	defer resp.Body.Close()
	if resp.StatusCode == http.StatusNotModified {
		return false, nil
	}
	if resp.StatusCode != http.StatusOK {
		return false, fmt.Errorf("server answered %s", resp.Status)
	}
	body, err := io.ReadAll(io.LimitReader(resp.Body, MaxFeed+1))
	if err != nil {
		return false, errors.New("download interrupted")
	}
	if len(body) > MaxFeed {
		return false, errors.New("calendar is too large")
	}
	if !strings.Contains(string(body[:min(len(body), 4096)]), "BEGIN:VCALENDAR") {
		return false, errors.New("that link isn't an iCal calendar")
	}
	if old, err := os.ReadFile(dest); err == nil && string(old) == string(body) {
		return false, nil
	}
	if err := writeAtomic(dest, body); err != nil {
		return false, err
	}
	if etag := resp.Header.Get("ETag"); etag != "" {
		_ = writeAtomic(dest+".etag", []byte(etag))
	}
	return true, nil
}

func writeAtomic(path string, data []byte) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		return err
	}
	tmp, err := os.CreateTemp(filepath.Dir(path), ".tmp-*")
	if err != nil {
		return err
	}
	defer os.Remove(tmp.Name())
	if _, err := tmp.Write(data); err != nil {
		tmp.Close()
		return err
	}
	if err := tmp.Close(); err != nil {
		return err
	}
	return os.Rename(tmp.Name(), path)
}

// Event is what the shell shows.
type Event struct {
	Title    string    `json:"title"`
	Location string    `json:"location,omitempty"`
	Start    time.Time `json:"start"`
	End      time.Time `json:"end"`
	AllDay   bool      `json:"all_day"`
	Calendar string    `json:"calendar"`
	Color    string    `json:"color"`
}

// Events lists every calendar's events overlapping [from, to), sorted.
// Calendars that can't be read (not downloaded yet, a bad file) are skipped.
func Events(d Dirs, srcs []Source, from, to time.Time, loc *time.Location) []Event {
	out := []Event{}
	for _, s := range srcs {
		p := s.Path
		if s.URL != "" {
			p = cachePath(d, s.URL)
		}
		f, err := os.Open(p)
		if err != nil {
			continue
		}
		items, err := ical.Parse(f, loc)
		f.Close()
		if err != nil {
			continue
		}
		for _, e := range ical.Between(items, from, to) {
			out = append(out, Event{Title: e.Title, Location: e.Location, Start: e.Start.In(loc), End: e.End.In(loc), AllDay: e.AllDay, Calendar: s.Name, Color: s.Color})
		}
	}
	sort.SliceStable(out, func(i, j int) bool {
		if !out[i].Start.Equal(out[j].Start) {
			return out[i].Start.Before(out[j].Start)
		}
		return out[i].AllDay && !out[j].AllDay
	})
	return out
}

// Version changes whenever anything the panel shows could have: a
// dropped-in file or a downloaded copy (settings changes are pushed
// separately). The shell re-reads when it does.
func Version(d Dirs, srcs []Source) int64 {
	var v int64
	bump := func(p string) {
		if fi, err := os.Stat(p); err == nil && fi.ModTime().UnixNano() > v {
			v = fi.ModTime().UnixNano()
		}
	}
	bump(filepath.Join(d.Data, "calendars"))
	bump(filepath.Join(d.Cache, "calendars"))
	for _, s := range srcs {
		if s.URL != "" {
			bump(cachePath(d, s.URL))
		} else {
			bump(s.Path)
		}
	}
	return v
}
