package calendar

import (
	"context"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

const feed = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:1\r\nSUMMARY:Design review\r\nDTSTART:20261109T150000Z\r\nDTEND:20261109T160000Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"

func dirs(t *testing.T) Dirs {
	root := t.TempDir()
	d := Dirs{Config: filepath.Join(root, "config"), Data: filepath.Join(root, "data"), Cache: filepath.Join(root, "cache"), Home: filepath.Join(root, "home")}
	for _, p := range []string{d.Config, d.Data, d.Cache, d.Home} {
		if err := os.MkdirAll(p, 0o755); err != nil {
			t.Fatal(err)
		}
	}
	return d
}

func write(t *testing.T, p, s string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(p), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(p, []byte(s), 0o644); err != nil {
		t.Fatal(err)
	}
}

func TestLinksAreFetchedCachedAndReadOffline(t *testing.T) {
	d := dirs(t)
	hits, notModified := 0, 0
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		hits++
		if r.Header.Get("If-None-Match") == `"v1"` {
			notModified++
			w.WriteHeader(http.StatusNotModified)
			return
		}
		w.Header().Set("ETag", `"v1"`)
		_, _ = w.Write([]byte(feed))
	}))
	defer srv.Close()
	secret := srv.URL + "/calendar/ical/private-abc123/basic.ics"
	srcs := Sources(d, []Source{{Name: "Personal", URL: secret}})

	changed, errs := Refresh(context.Background(), d, srcs, srv.Client())
	if len(errs) != 0 || changed != 1 {
		t.Fatalf("first refresh: changed %d, errs %v", changed, errs)
	}
	changed, errs = Refresh(context.Background(), d, srcs, srv.Client())
	if len(errs) != 0 || changed != 0 || notModified != 1 {
		t.Fatalf("second refresh should be a 304: changed %d errs %v 304s %d", changed, errs, notModified)
	}
	srv.Close() // offline from here on
	_, errs = Refresh(context.Background(), d, srcs, http.DefaultClient)
	if len(errs) != 1 || strings.Contains(errs[0].Error(), "private-abc123") {
		t.Fatalf("offline: errors %v (and the secret link must never appear)", errs)
	}
	evs := Events(d, srcs, time.Date(2026, 11, 1, 0, 0, 0, 0, time.UTC), time.Date(2026, 12, 1, 0, 0, 0, 0, time.UTC), time.UTC)
	if len(evs) != 1 || evs[0].Title != "Design review" || evs[0].Calendar != "Personal" || evs[0].Color != Palette[0] {
		t.Fatalf("events from the cache: %+v", evs)
	}
	// Cache files are named by hash, not by the secret link.
	entries, _ := os.ReadDir(filepath.Join(d.Cache, "calendars"))
	for _, e := range entries {
		if strings.Contains(e.Name(), "abc123") {
			t.Fatalf("cache file named after the link: %s", e.Name())
		}
	}
}

func TestBadLinksAreRefusedWithoutTouchingTheCache(t *testing.T) {
	d := dirs(t)
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if strings.HasSuffix(r.URL.Path, "/html") {
			_, _ = w.Write([]byte("<html>login page</html>"))
			return
		}
		w.WriteHeader(http.StatusForbidden)
	}))
	defer srv.Close()
	srcs := []Source{{Name: "A", URL: srv.URL + "/html"}, {Name: "B", URL: srv.URL + "/x"}, {Name: "C", URL: "ftp://example.com/x.ics"}}
	changed, errs := Refresh(context.Background(), d, srcs, srv.Client())
	if changed != 0 || len(errs) != 3 {
		t.Fatalf("changed %d errs %v", changed, errs)
	}
	for _, want := range []string{"isn't an iCal calendar", "403", "only http(s)"} {
		found := false
		for _, e := range errs {
			found = found || strings.Contains(e.Error(), want)
		}
		if !found {
			t.Errorf("no error mentioning %q in %v", want, errs)
		}
	}
}

func TestFilesDropInsColoursAndVersion(t *testing.T) {
	d := dirs(t)
	write(t, filepath.Join(d.Home, "Documents", "work.ics"), feed)
	write(t, filepath.Join(d.Data, "calendars", "Holidays.ics"), strings.Replace(feed, "Design review", "Bank holiday", 1))
	srcs := Sources(d, []Source{{Name: "Work", Path: "~/Documents/work.ics", Color: "#123456"}})
	if len(srcs) != 2 || srcs[0].Color != "#123456" || srcs[1].Name != "Holidays" || srcs[1].Color != Palette[1] {
		t.Fatalf("sources: %+v", srcs)
	}
	v1 := Version(d, srcs)
	evs := Events(d, srcs, time.Date(2026, 11, 9, 0, 0, 0, 0, time.UTC), time.Date(2026, 11, 10, 0, 0, 0, 0, time.UTC), time.UTC)
	if len(evs) != 2 {
		t.Fatalf("events: %+v", evs)
	}
	later := time.Now().Add(2 * time.Second)
	_ = os.Chtimes(filepath.Join(d.Home, "Documents", "work.ics"), later, later)
	if Version(d, srcs) == v1 {
		t.Fatal("editing a calendar file must change the version")
	}
}

func TestNoCalendarsMeansNoEvents(t *testing.T) {
	d := dirs(t)
	if evs := Events(d, Sources(d, nil), time.Now(), time.Now().Add(time.Hour), time.UTC); len(evs) != 0 {
		t.Fatalf("%v", evs)
	}
}
