//go:build uitest

// Click-and-check tests for Files: run with `make ui-test` (they need a
// Wayland session to run a nested WhiteRose in, and the programs built).
package uitest_test

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"whiterose/daemon/internal/uitest"
)

func filesBinary(s *uitest.Session) string {
	if p := os.Getenv("WR_UITEST_FILES"); p != "" {
		return p
	}
	return filepath.Join(s.Repo, "apps/target/debug/whiterose-files")
}

// Rename many: select the photos, F2, type a rule; the list shows the new
// names as you type, and Rename renames them on disk.
func TestRenameManyShowsNewNamesThenRenames(t *testing.T) {
	s := uitest.Start(t, uitest.Options{})
	dir := filepath.Join(s.Home, "Pictures", "trip")
	os.MkdirAll(dir, 0o755)
	for _, n := range []string{"IMG_0412.JPG", "IMG_0413.JPG"} {
		os.WriteFile(filepath.Join(dir, n), []byte("x"), 0o644)
	}
	files := s.Open(filesBinary(s)+" "+dir, "trip - Files")

	files.Press("ctrl+2") // the list (where the new names show)
	files.WaitFor(uitest.Named("IMG_0412.JPG").Role("list item"))
	files.Click(uitest.Named("IMG_0412.JPG").Role("list item"))
	files.Press("ctrl+a")
	files.Press("F2")
	files.WaitFor(uitest.Named("Rename").Role("page tab"))

	files.Click(uitest.Named("Find").Role("text"))
	files.Type("IMG_")
	files.Click(uitest.Named("Replace with").Role("text"))
	files.Type("lab-day ")
	files.WaitFor(uitest.Named("IMG_0412.JPG").Described("→ lab-day 0412.JPG"))
	files.WaitFor(uitest.Named("IMG_0413.JPG").Described("→ lab-day 0413.JPG"))
	if got := files.Text(uitest.Named("Find").Role("text")); got != "IMG_" {
		t.Errorf("the Find box holds %q", got)
	}

	files.Click(uitest.Named("Rename 2").Role("push button"))
	files.WaitFor(uitest.Named("lab-day 0412.JPG").Role("list item"))
	for _, n := range []string{"lab-day 0412.JPG", "lab-day 0413.JPG"} {
		if _, err := os.Stat(filepath.Join(dir, n)); err != nil {
			t.Errorf("%s isn't on disk: %v", n, err)
		}
	}
}

// A new folder appears in the list with its name ready to type: Enter
// makes it, Esc doesn't.
func TestNewFolderIsNamedInPlace(t *testing.T) {
	s := uitest.Start(t, uitest.Options{})
	dir := filepath.Join(s.Home, "Documents", "notes")
	os.MkdirAll(dir, 0o755)
	os.WriteFile(filepath.Join(dir, "todo.txt"), []byte("x"), 0o644)
	files := s.Open(filesBinary(s)+" "+dir, "notes - Files")
	files.Press("ctrl+2")
	files.Click(uitest.Named("todo.txt").Role("list item"))

	files.Press("ctrl+shift+n")
	files.WaitFor(uitest.Named("Name").Role("text"))
	files.Press("ctrl+a")
	files.Type("Drafts")
	files.Press("Return")
	files.WaitFor(uitest.Named("Drafts").Role("list item"))
	if fi, err := os.Stat(filepath.Join(dir, "Drafts")); err != nil || !fi.IsDir() {
		t.Fatalf("Drafts isn't a folder on disk: %v", err)
	}

	files.Press("ctrl+shift+n")
	files.WaitFor(uitest.Named("Name").Role("text"))
	files.Type("Nope")
	files.Press("Escape")
	files.Gone(uitest.Named("Name").Role("text"))
	if _, err := os.Stat(filepath.Join(dir, "New folderNope")); err == nil {
		t.Error("Esc still made a folder")
	}
	entries, _ := os.ReadDir(dir)
	if len(entries) != 2 {
		t.Errorf("want todo.txt and Drafts, got %d entries", len(entries))
	}
}

// Files makes the search index in a process of its own: closing Files
// straight away doesn't stop it, and Files opened again says it's indexed.
func TestIndexCarriesOnWhenFilesCloses(t *testing.T) {
	s := uitest.Start(t, uitest.Options{})
	for i := range 40 {
		dir := filepath.Join(s.Home, "Documents", fmt.Sprintf("project-%02d", i))
		os.MkdirAll(dir, 0o755)
		for j := range 250 {
			os.WriteFile(filepath.Join(dir, fmt.Sprintf("note-%03d.txt", j)), nil, 0o644)
		}
	}
	index := filepath.Join(s.Home, ".cache/whiterose/files/index")
	title := "Home - Files"
	files := s.Open(filesBinary(s)+" "+s.Home, title)
	files.Click(uitest.Named("Close").Role("push button"))
	files.Closed()

	deadline := time.Now().Add(20 * time.Second)
	for {
		_, err := os.Stat(index)
		_, busy := os.Stat(filepath.Join(s.Home, ".cache/whiterose/files/index.progress"))
		if err == nil && os.IsNotExist(busy) {
			break
		}
		if time.Now().After(deadline) {
			t.Fatalf("no index after Files closed (index: %v, progress file: %v)", err, busy)
		}
		time.Sleep(100 * time.Millisecond)
	}

	// Searching home: the index row says when it was made.
	files = s.Open(filesBinary(s)+" "+s.Home, title)
	files.Press("ctrl+f")
	files.Type("note-007")
	files = files.Retitled("Search - Files")
	files.Click(uitest.Named("Home").Role("radio button"))
	files.WaitFor(uitest.Named("Index again").Role("push button"))
}

// pick runs `whiterose-files --choose args` in the session and returns a
// wait for what it printed and its exit code.
func pick(t *testing.T, s *uitest.Session, args string) func() (string, string) {
	out := filepath.Join(s.Home, fmt.Sprintf("pick-%d", time.Now().UnixNano()))
	script := out + ".sh"
	os.WriteFile(script, []byte(fmt.Sprintf("%s --choose %s > %s.out; echo $? > %s.code\n", filesBinary(s), args, out, out)), 0o755)
	s.Spawn("sh " + script)
	return func() (string, string) {
		for range 100 {
			if code, err := os.ReadFile(out + ".code"); err == nil {
				printed, _ := os.ReadFile(out + ".out")
				return string(printed), strings.TrimSpace(string(code))
			}
			time.Sleep(100 * time.Millisecond)
		}
		t.Fatal("the picker didn't finish")
		return "", ""
	}
}

// The picker: only what the filter allows is offered; Open prints the
// chosen file; a folder can be chosen; saving over a file asks first;
// Cancel prints nothing and exits 1.
func TestFilesPicker(t *testing.T) {
	s := uitest.Start(t, uitest.Options{})
	dir := filepath.Join(s.Home, "Pictures")
	os.MkdirAll(filepath.Join(dir, "trip"), 0o755)
	for _, n := range []string{"a.png", "notes.txt"} {
		os.WriteFile(filepath.Join(dir, n), []byte("x"), 0o644)
	}

	done := pick(t, s, "--title 'Choose a picture' --in "+dir+" --filter 'Images:*.png *.jpg'")
	w := s.WaitWindow("Choose a picture - Files")
	w.Press("ctrl+2")
	w.WaitFor(uitest.Named("a.png").Role("list item"))
	w.WaitFor(uitest.Named("trip").Role("list item"))
	if _, ok := w.Lookup(uitest.Named("notes.txt").Role("list item")); ok {
		t.Error("notes.txt is offered for a picture")
	}
	w.Click(uitest.Named("a.png").Role("list item"))
	w.Click(uitest.Named("Open").Role("push button"))
	if out, code := done(); code != "0" || out != filepath.Join(dir, "a.png")+"\n" {
		t.Errorf("open: printed %q, exit %s", out, code)
	}

	done = pick(t, s, "--folder --in "+dir)
	w = s.WaitWindow("Choose a folder - Files")
	w.Click(uitest.Named("Choose folder").Role("push button"))
	if out, code := done(); code != "0" || out != dir+"\n" {
		t.Errorf("folder: printed %q, exit %s", out, code)
	}

	done = pick(t, s, "--save notes.txt --in "+dir+" --json")
	w = s.WaitWindow("Save as - Files")
	w.Click(uitest.Named("Save").Role("push button"))
	w.WaitFor(uitest.Named("Replace").Role("push button"))
	w.Click(uitest.Named("Replace").Role("push button"))
	if out, code := done(); code != "0" || out != `["`+filepath.Join(dir, "notes.txt")+`"]`+"\n" {
		t.Errorf("save: printed %q, exit %s", out, code)
	}

	done = pick(t, s, "--in "+dir)
	w = s.WaitWindow("Choose a file - Files")
	w.Click(uitest.Named("Cancel").Role("push button"))
	if out, code := done(); code != "1" || out != "" {
		t.Errorf("cancel: printed %q, exit %s", out, code)
	}
}
