// Package activityfiles records which activity was current when each
// recently used file was used, so the launcher and the overview list
// only the current activity's (0.4 activities). recently-used.xbel, which
// apps write, has no field for it: the daemon watches the file, and a
// bookmark that's new or used again since it last looked is tagged with
// the activity current then. The tags ({path: activity id}) are the
// "activities.recentFiles" setting; a file no longer in recently-used.xbel
// loses its tag. Files used before the daemon started (or while
// activities were off) have none, and count as the default activity's.
package activityfiles

import (
	"bytes"
	"encoding/xml"
	"errors"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"time"
	"unsafe"

	"golang.org/x/sys/unix"
)

// File is where apps keep the recently used files.
func File() string {
	dir := os.Getenv("XDG_DATA_HOME")
	if dir == "" {
		home, _ := os.UserHomeDir()
		dir = filepath.Join(home, ".local", "share")
	}
	return filepath.Join(dir, "recently-used.xbel")
}

// A Bookmark is one local file in recently-used.xbel and when it was last
// used (its latest of modified and visited, as written: ISO 8601 UTC, so
// they compare as strings).
type Bookmark struct {
	Path string
	When string
}

// Parse reads the local files in recently-used.xbel.
func Parse(data []byte) ([]Bookmark, error) {
	var doc struct {
		Bookmarks []struct {
			Href     string `xml:"href,attr"`
			Modified string `xml:"modified,attr"`
			Visited  string `xml:"visited,attr"`
		} `xml:"bookmark"`
	}
	if err := xml.NewDecoder(bytes.NewReader(data)).Decode(&doc); err != nil {
		return nil, err
	}
	out := make([]Bookmark, 0, len(doc.Bookmarks))
	for _, b := range doc.Bookmarks {
		if !strings.HasPrefix(b.Href, "file://") {
			continue
		}
		path, err := url.PathUnescape(strings.TrimPrefix(b.Href, "file://"))
		if err != nil || path == "" {
			continue
		}
		when := b.Modified
		if b.Visited > when {
			when = b.Visited
		}
		out = append(out, Bookmark{Path: path, When: when})
	}
	return out, nil
}

// A Tagger remembers when it last saw each file used.
type Tagger struct {
	seen map[string]string
}

// Update takes the bookmarks now and the tags so far. The first time it
// only learns what's there. After that a file new or used again is
// tagged `current` (when that's not ""); a file gone loses its tag. It
// returns the tags and whether they changed.
func (t *Tagger) Update(books []Bookmark, current string, tags map[string]string) (map[string]string, bool) {
	first := t.seen == nil
	if first {
		t.seen = map[string]string{}
	}
	out := map[string]string{}
	changed := false
	here := map[string]bool{}
	for _, b := range books {
		here[b.Path] = true
		if tag, ok := tags[b.Path]; ok {
			out[b.Path] = tag
		}
		if !first && current != "" && b.When > t.seen[b.Path] && out[b.Path] != current {
			out[b.Path] = current
			changed = true
		}
		t.seen[b.Path] = b.When
	}
	for p := range tags {
		if !here[p] {
			changed = true
		}
	}
	for p := range t.seen {
		if !here[p] {
			delete(t.seen, p)
		}
	}
	return out, changed
}

// Watch calls changed (after a short pause, so a burst of writes is one
// call) whenever the file at path is written or replaced, until stop is
// closed. The folder must exist.
func Watch(path string, changed func(), stop <-chan struct{}) error {
	fd, err := unix.InotifyInit1(unix.IN_CLOEXEC)
	if err != nil {
		return err
	}
	defer unix.Close(fd)
	if _, err := unix.InotifyAddWatch(fd, filepath.Dir(path), unix.IN_CLOSE_WRITE|unix.IN_MOVED_TO); err != nil {
		return err
	}
	go func() {
		<-stop
		unix.Close(fd)
	}()
	name := filepath.Base(path)
	var timer *time.Timer
	buf := make([]byte, 4096)
	for {
		n, err := unix.Read(fd, buf)
		if err != nil || n <= 0 {
			select {
			case <-stop:
				return nil
			default:
			}
			if errors.Is(err, unix.EINTR) {
				continue
			}
			return err
		}
		for off := 0; off+unix.SizeofInotifyEvent <= n; {
			ev := (*unix.InotifyEvent)(unsafe.Pointer(&buf[off]))
			nameBytes := buf[off+unix.SizeofInotifyEvent : off+unix.SizeofInotifyEvent+int(ev.Len)]
			off += unix.SizeofInotifyEvent + int(ev.Len)
			if strings.TrimRight(string(nameBytes), "\x00") != name {
				continue
			}
			if timer != nil {
				timer.Stop()
			}
			timer = time.AfterFunc(500*time.Millisecond, changed)
		}
	}
}
