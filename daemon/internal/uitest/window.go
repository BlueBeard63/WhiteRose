package uitest

import (
	"strings"
	"time"
)

// Window is an app's window in the session, found by its title.
type Window struct {
	s     *Session
	title string
}

// How long things get to appear (an app starting, a view opening).
var Patience = 10 * time.Second

// Open starts `command` in the session and waits for its window titled
// `title` (on screen, with its items in the accessibility tree).
func (s *Session) Open(command, title string) *Window {
	s.t.Helper()
	s.Spawn(command)
	w := &Window{s: s, title: title}
	a := s.access()
	var onScreen, inTree bool
	if !waitUntil(Patience+10*time.Second, func() bool {
		_, onScreen = s.window(title)
		_, inTree = a.frame(title)
		return onScreen && inTree
	}) {
		s.t.Fatalf("no window titled %q ready (on screen: %v, in the accessibility tree: %v; windows: %s; accessible apps: %s)",
			title, onScreen, inTree, s.titles(), a.apps())
	}
	return w
}

func (s *Session) titles() string {
	s.mu.Lock()
	defer s.mu.Unlock()
	var t []string
	for _, w := range s.windows {
		if w.Title != nil {
			t = append(t, *w.Title)
		}
	}
	return strings.Join(t, ", ")
}

// Retitled follows the window after its title changed (Files: the folder).
func (w *Window) Retitled(title string) *Window {
	w.s.t.Helper()
	n := &Window{s: w.s, title: title}
	if !waitUntil(Patience, func() bool { _, ok := w.s.window(title); return ok }) {
		w.s.t.Fatalf("the window didn't become %q (windows: %s)", title, w.s.titles())
	}
	return n
}

// Nodes is everything showing in the window.
func (w *Window) Nodes() []Node {
	f, ok := w.s.access().frame(w.title)
	if !ok {
		return nil
	}
	return w.s.a11y.all(f)
}

// Find returns what matches q now (the first, in the order they're shown).
func (w *Window) Find(q Query) (Node, bool) {
	for _, n := range w.Nodes() {
		if q.matches(n) {
			return n, true
		}
	}
	return Node{}, false
}

// WaitFor waits until something matching q is showing, and returns it; the
// test fails (listing what is showing) if it never does.
func (w *Window) WaitFor(q Query) Node {
	w.s.t.Helper()
	var found Node
	if !waitUntil(Patience, func() bool { n, ok := w.Find(q); found = n; return ok }) {
		w.s.t.Fatalf("%s never showed in %q; showing:\n%s", q, w.title, describe(w.Nodes()))
	}
	return found
}

// Gone waits until nothing matching q is showing.
func (w *Window) Gone(q Query) {
	w.s.t.Helper()
	if !waitUntil(Patience, func() bool { _, ok := w.Find(q); return !ok }) {
		w.s.t.Fatalf("%s is still showing in %q", q, w.title)
	}
}

// Click clicks the middle of what matches q with the pointer.
func (w *Window) Click(q Query) Node {
	w.s.t.Helper()
	n := w.WaitFor(q)
	w.clickAt(n, false)
	return n
}

// DoubleClick double-clicks it.
func (w *Window) DoubleClick(q Query) Node {
	w.s.t.Helper()
	n := w.WaitFor(q)
	w.clickAt(n, true)
	return n
}

// RightClick right-clicks it (its menu).
func (w *Window) RightClick(q Query) Node {
	w.s.t.Helper()
	n := w.WaitFor(q)
	x, y := w.middle(n)
	w.s.pointer(x, y, "")
	w.s.pointer(x, y, "right-press")
	w.s.pointer(x, y, "right-release")
	return n
}

func (w *Window) middle(n Node) (float64, float64) {
	w.s.t.Helper()
	win, ok := w.s.window(w.title)
	if !ok {
		w.s.t.Fatalf("the window %q isn't on screen", w.title)
	}
	g := *win.Geometry
	return float64(g[0] + n.X + n.W/2), float64(g[1] + n.Y + n.H/2)
}

func (w *Window) clickAt(n Node, double bool) {
	x, y := w.middle(n)
	w.s.pointer(x, y, "")
	clicks := 1
	if double {
		clicks = 2
	}
	for range clicks {
		w.s.pointer(x, y, "press")
		w.s.pointer(x, y, "release")
	}
	time.Sleep(150 * time.Millisecond)
}

// Text is what the field matching q holds.
func (w *Window) Text(q Query) string {
	w.s.t.Helper()
	return w.s.a11y.text(w.WaitFor(q))
}

// Type types text with the keyboard (into whatever has focus).
func (w *Window) Type(text string) { w.s.typeText(text) }

// Press presses keys together: "Return", "ctrl+a", "shift+Delete", "F2".
func (w *Window) Press(combo string) { w.s.press(combo) }
