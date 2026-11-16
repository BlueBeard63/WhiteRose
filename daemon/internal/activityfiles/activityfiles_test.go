package activityfiles

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

const xbel = `<?xml version="1.0" encoding="UTF-8"?>
<xbel version="1.0" xmlns:bookmark="http://www.freedesktop.org/standards/desktop-bookmarks">
  <bookmark href="file:///home/u/notes%20a.md" added="2026-11-01T10:00:00Z" modified="2026-11-01T10:00:00Z" visited="2026-11-02T09:00:00Z"/>
  <bookmark href="file:///home/u/b.pdf" added="2026-11-01T10:00:00Z" modified="2026-11-03T10:00:00Z" visited="2026-11-01T10:00:00Z"/>
  <bookmark href="sftp://host/c.txt" added="2026-11-01T10:00:00Z" modified="2026-11-03T10:00:00Z" visited="2026-11-01T10:00:00Z"/>
</xbel>`

func TestParseTakesLocalFilesAndTheirLatestUse(t *testing.T) {
	b, err := Parse([]byte(xbel))
	if err != nil {
		t.Fatal(err)
	}
	want := []Bookmark{{"/home/u/notes a.md", "2026-11-02T09:00:00Z"}, {"/home/u/b.pdf", "2026-11-03T10:00:00Z"}}
	if len(b) != 2 || b[0] != want[0] || b[1] != want[1] {
		t.Fatalf("got %v", b)
	}
}

func TestTaggerTagsWhatsUsedAfterItStarts(t *testing.T) {
	var tg Tagger
	start := []Bookmark{{"/a", "1"}, {"/b", "1"}}
	tags, changed := tg.Update(start, "home", map[string]string{"/b": "work"})
	if changed || len(tags) != 1 || tags["/b"] != "work" {
		t.Fatalf("the first look only learns: %v %v", tags, changed)
	}
	// /a used again in Home, /c new in Home; /b untouched.
	tags, changed = tg.Update([]Bookmark{{"/a", "2"}, {"/b", "1"}, {"/c", "2"}}, "home", tags)
	if !changed || tags["/a"] != "home" || tags["/c"] != "home" || tags["/b"] != "work" {
		t.Fatalf("got %v %v", tags, changed)
	}
	// Nothing new: nothing changes. Activities off (""): nothing tagged.
	if _, changed = tg.Update([]Bookmark{{"/a", "2"}, {"/b", "1"}, {"/c", "2"}}, "home", tags); changed {
		t.Fatal("changed with nothing used")
	}
	if tags2, _ := tg.Update([]Bookmark{{"/a", "3"}, {"/b", "1"}, {"/c", "2"}}, "", tags); tags2["/a"] != "home" {
		t.Fatalf("retagged while off: %v", tags2)
	}
	// /b gone from the list: its tag goes.
	tags, changed = tg.Update([]Bookmark{{"/a", "3"}, {"/c", "2"}}, "work", tags)
	if !changed || len(tags) != 2 {
		t.Fatalf("got %v %v", tags, changed)
	}
}

func TestWatchSeesTheFileReplaced(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "recently-used.xbel")
	got := make(chan struct{}, 4)
	stop := make(chan struct{})
	go Watch(path, func() { got <- struct{}{} }, stop)
	defer close(stop)
	time.Sleep(100 * time.Millisecond)
	// As apps do: write elsewhere, rename over it.
	tmp := filepath.Join(dir, "x.tmp")
	os.WriteFile(tmp, []byte(xbel), 0o600)
	os.Rename(tmp, path)
	os.WriteFile(filepath.Join(dir, "other"), nil, 0o600)
	select {
	case <-got:
	case <-time.After(3 * time.Second):
		t.Fatal("not told")
	}
}
