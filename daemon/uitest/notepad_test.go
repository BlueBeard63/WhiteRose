//go:build uitest

package uitest_test

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"whiterose/daemon/internal/uitest"
)

func notepadBinary(s *uitest.Session) string {
	if p := os.Getenv("WR_UITEST_NOTEPAD"); p != "" {
		return p
	}
	return filepath.Join(s.Repo, "apps/target/debug/whiterose-notepad")
}

func TestNotepadOpensFilesInTheExistingWindow(t *testing.T) {
	s := uitest.Start(t, uitest.Options{})
	first := filepath.Join(s.Home, "first.rs")
	second := filepath.Join(s.Home, "second note.md")
	for path, text := range map[string]string{
		first:  "fn main() {}\n",
		second: "<b>Literal text, not formatting</b>\n",
	} {
		if err := os.WriteFile(path, []byte(text), 0o600); err != nil {
			t.Fatal(err)
		}
	}
	// Existing sessions still restore, but only in the initial window.
	sessionDir := filepath.Join(s.Home, ".local/share/whiterose/notepad")
	if err := os.MkdirAll(sessionDir, 0o700); err != nil {
		t.Fatal(err)
	}
	session, _ := json.Marshal(map[string]any{
		"tabs": []map[string]any{{"path": first, "cursor": 0}}, "current": 1,
	})
	if err := os.WriteFile(filepath.Join(sessionDir, "session.json"), session, 0o600); err != nil {
		t.Fatal(err)
	}
	binary := notepadBinary(s)
	s.Launch("whiterose-notepad", binary, first)
	note := s.WaitWindow("first.rs - Notepad")
	note.WaitFor(uitest.Named("Document").Role("text"))
	// A restored, highlighted file must not become dirty by itself.
	time.Sleep(time.Second)
	if s.HasWindow("first.rs • - Notepad") {
		t.Fatal("loading/highlighting marked the unedited file dirty")
	}
	note.Press("ctrl+End")
	note.Type("x")
	note = note.Retitled("first.rs • - Notepad")

	s.Launch("whiterose-notepad", binary, second)
	note = note.Retitled("second note.md - Notepad")
	if count := s.AppWindowCount("whiterose-notepad"); count != 1 {
		t.Fatalf("opening a second file created %d windows, want 1", count)
	}
	note.WaitFor(uitest.Named("first.rs").Role("page tab"))
	note.WaitFor(uitest.Named("second note.md").Role("page tab"))
	if got := note.Text(uitest.Named("Document").Role("text")); strings.ReplaceAll(got, "\u2029", "\n") != "<b>Literal text, not formatting</b>\n" {
		t.Fatalf("loading a text file interpreted its contents as rich text: %q", got)
	}
	// Reopening a file selects its existing tab without reloading edits.
	s.Launch("whiterose-notepad", binary, first)
	note = note.Retitled("first.rs • - Notepad")
	if got := note.Text(uitest.Named("Document").Role("text")); strings.ReplaceAll(got, "\u2029", "\n") != "fn main() {}\nx" {
		t.Fatalf("opening the same file lost unsaved edits: %q", got)
	}
	tabs := 0
	for _, n := range note.Nodes() {
		if n.Role == "page tab" {
			tabs++
		}
	}
	if tabs != 3 {
		t.Fatalf("got %d tabs, want Scratch + first.rs + second note.md", tabs)
	}
	// Undo returns to the loaded text, so the unsaved marker goes away.
	note.Press("ctrl+z")
	note = note.Retitled("first.rs - Notepad")
	note.Press("ctrl+s")
	if data, err := os.ReadFile(first); err != nil || string(data) != "fn main() {}\n" {
		t.Fatalf("the file's content changed unexpectedly: %q (%v)", data, err)
	}
	// Launching without files activates, rather than restoring another copy.
	note.Click(uitest.Named("Minimise").Role("push button"))
	s.Launch("whiterose-notepad", binary)
	note = s.WaitWindow("first.rs - Notepad")
	if !uitest.WaitUntil(func() bool {
		_, focused, ok := s.Toplevel("first.rs - Notepad")
		return ok && focused
	}) {
		t.Fatal("relaunching didn't activate the existing window")
	}
	if count := s.AppWindowCount("whiterose-notepad"); count != 1 {
		t.Fatalf("relaunching created %d windows, want 1", count)
	}
}

func TestNotepadEmptyDocumentStartsClean(t *testing.T) {
	s := uitest.Start(t, uitest.Options{})
	s.Launch("whiterose-notepad", notepadBinary(s))
	note := s.WaitWindow("Scratch - Notepad")
	note.Press("ctrl+n")
	note = note.Retitled("Untitled - Notepad")
	note.WaitFor(uitest.Named("New document"))
	time.Sleep(time.Second)
	if s.HasWindow("Untitled • - Notepad") {
		t.Fatal("an untouched new document is marked dirty")
	}
	note.Type("changed")
	note = note.Retitled("Untitled • - Notepad")
	note.WaitFor(uitest.Named("Not saved yet"))
	note.Press("ctrl+w")
	note.WaitFor(uitest.Named("Don't save").Role("push button"))
	note.Click(uitest.Named("Cancel").Role("push button"))
	note.Press("ctrl+z")
	note = note.Retitled("Untitled - Notepad")
	note.WaitFor(uitest.Named("New document"))
	// No unsaved-changes prompt for an empty, untouched (or undone) tab.
	note.Press("ctrl+w")
	note = note.Retitled("Scratch - Notepad")
	note.Click(uitest.Named("Close").Role("push button"))
	note.Closed()
}
