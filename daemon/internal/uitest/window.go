package uitest

import (
	"path/filepath"
	"strings"
	"time"
)

// Window is an app's window in the session, found by its title, or one of
// the shell's layer surfaces (the bar), found by its namespace.
type Window struct {
	s     *Session
	title string
	// The layer surface's namespace ("" for an app's window); the shell
	// titles its window the same (LayerTitle.qml).
	layer string
}

// How long things get to appear (an app starting, a view opening).
var Patience = 10 * time.Second

// Open starts `command` in the session and waits for its window titled
// `title` (on screen, with its items in the accessibility tree).
func (s *Session) Open(command, title string) *Window {
	s.t.Helper()
	s.Spawn(command)
	return s.WaitWindow(title)
}

// WaitWindow waits for a window titled `title` that something else opened
// (Settings from the shell).
func (s *Session) WaitWindow(title string) *Window {
	s.t.Helper()
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

// Settings opens WhiteRose Settings (at `page` if not ""), as the shell does.
func (s *Session) Settings(page string) *Window {
	s.t.Helper()
	cmd := filepath.Join(s.Repo, "shell/bin/whiterose-settings")
	if page != "" {
		cmd += " " + page
	}
	return s.Open(cmd, "Settings")
}

// Layer is the shell's layer surface named `namespace` ("whiterose-bar":
// the bar, with the island and its panels), once it's on screen and in
// the accessibility tree. Needs Options.Shell.
func (s *Session) Layer(namespace string) *Window {
	s.t.Helper()
	a := s.access()
	var onScreen, inTree bool
	if !waitUntil(Patience+10*time.Second, func() bool {
		_, onScreen = s.layer(namespace)
		_, inTree = a.frame(namespace)
		return onScreen && inTree
	}) {
		s.t.Fatalf("no layer %q ready (on screen: %v, in the accessibility tree: %v; layers: %s; accessible apps: %s)",
			namespace, onScreen, inTree, s.layerNames(), a.apps())
	}
	return &Window{s: s, title: namespace, layer: namespace}
}

// Bar is the shell's top bar (and the island and panels that hang from it).
func (s *Session) Bar() *Window { return s.Layer("whiterose-bar") }

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

// Closed waits until the window is gone (from the screen and the tree).
func (w *Window) Closed() {
	w.s.t.Helper()
	if !waitUntil(Patience, func() bool {
		_, onScreen := w.s.window(w.title)
		_, inTree := w.s.access().frame(w.title)
		return !onScreen && !inTree
	}) {
		w.s.t.Fatalf("the window %q didn't close (windows: %s)", w.title, w.s.titles())
	}
}

// Nodes is everything showing in the window.
func (w *Window) Nodes() []Node {
	f, ok := w.s.access().frame(w.title)
	if !ok {
		return nil
	}
	return w.s.a11y.all(f, false)
}

// Laid out is everything in the window that has a size, showing or
// scrolled out of sight.
func (w *Window) LaidOut() []Node {
	f, ok := w.s.access().frame(w.title)
	if !ok {
		return nil
	}
	var out []Node
	for _, n := range w.s.a11y.all(f, true) {
		if n.W > 0 && n.H > 0 {
			out = append(out, n)
		}
	}
	return out
}

// Lookup is what matches q in the window now, showing or scrolled out of
// sight (the one showing, if several match).
func (w *Window) Lookup(q Query) (Node, bool) {
	var found Node
	var ok bool
	for _, n := range w.LaidOut() {
		if q.matches(n) {
			if n.Showing() {
				return n, true
			}
			if !ok {
				found, ok = n, true
			}
		}
	}
	return found, ok
}

// Activate does what matches q through the accessibility tree (as a
// screen reader would), not the pointer: for things scrolled out of sight.
func (w *Window) Activate(q Query) Node {
	w.s.t.Helper()
	var found Node
	if !waitUntil(Patience, func() bool { n, ok := w.Lookup(q); found = n; return ok }) {
		w.s.t.Fatalf("%s isn't in %q", q, w.title)
	}
	if !w.s.a11y.act(found) {
		w.s.t.Fatalf("%s can't be pressed", q)
	}
	time.Sleep(150 * time.Millisecond)
	return found
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

// ScrollTo waits for what matches q and scrolls it into sight with the
// wheel (over the window, above or below the middle as it needs), as a
// person would; then returns it.
func (w *Window) ScrollTo(q Query) Node {
	w.s.t.Helper()
	var found Node
	if !waitUntil(Patience, func() bool { n, ok := w.Lookup(q); found = n; return ok }) {
		w.s.t.Fatalf("%s isn't in %q; showing:\n%s", q, w.title, describe(w.Nodes()))
	}
	var g [4]int
	for range 60 {
		// (Each time: a layer grows as what it shows opens, e.g. the bar
		// as a panel slides down.)
		g = w.geometry()
		if n, ok := w.Find(q); ok && n.Y >= 0 && n.Y+n.H <= g[3] {
			// Still gliding from the wheel? (A click would only stop it.)
			time.Sleep(150 * time.Millisecond)
			if m, ok := w.Find(q); ok && m.X == n.X && m.Y == n.Y {
				return m
			}
			continue
		}
		clicks := 2
		if found.Y < g[3]/2 {
			clicks = -2
		}
		w.s.scroll(float64(g[0]+found.X+found.W/2), float64(g[1]+g[3]/2), clicks)
		if n, ok := w.Lookup(q); ok {
			found = n
		}
	}
	w.s.t.Fatalf("couldn't scroll to %s in %q (it's at %d,%d %dx%d, showing: %v; the window is %dx%d)",
		q, w.title, found.X, found.Y, found.W, found.H, found.Showing(), g[2], g[3])
	return found
}

// Click clicks the middle of what matches q with the pointer (scrolling
// to it first if it's out of sight).
func (w *Window) Click(q Query) Node {
	w.s.t.Helper()
	n := w.ScrollTo(q)
	w.clickAt(n, false)
	return n
}

// DoubleClick double-clicks it.
func (w *Window) DoubleClick(q Query) Node {
	w.s.t.Helper()
	n := w.ScrollTo(q)
	w.clickAt(n, true)
	return n
}

// Hover rests the pointer on the middle of what matches q.
func (w *Window) Hover(q Query) Node {
	w.s.t.Helper()
	n := w.ScrollTo(q)
	x, y := w.middle(n)
	w.s.pointer(x, y, "")
	return n
}

// RightClick right-clicks it (its menu).
func (w *Window) RightClick(q Query) Node {
	w.s.t.Helper()
	n := w.ScrollTo(q)
	x, y := w.middle(n)
	w.s.pointer(x, y, "")
	w.s.pointer(x, y, "right-press")
	w.s.pointer(x, y, "right-release")
	return n
}

func (w *Window) middle(n Node) (float64, float64) {
	w.s.t.Helper()
	g := w.geometry()
	return float64(g[0] + n.X + n.W/2), float64(g[1] + n.Y + n.H/2)
}

// Geometry is where the window is on screen: x, y, width, height.
func (w *Window) Geometry() [4]int {
	w.s.t.Helper()
	return w.geometry()
}

// geometry is where the window is on screen: x, y, width, height.
func (w *Window) geometry() [4]int {
	w.s.t.Helper()
	var g [4]int
	if w.layer != "" {
		l, ok := w.s.layer(w.layer)
		if !ok {
			w.s.t.Fatalf("the layer %q isn't on screen", w.layer)
		}
		g = l.Geometry
	} else {
		win, ok := w.s.window(w.title)
		if !ok {
			w.s.t.Fatalf("the window %q isn't on screen", w.title)
		}
		g = *win.Geometry
	}
	return g
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

// WaitUntil waits (Patience) until ok says yes; false if it never did.
func WaitUntil(ok func() bool) bool { return waitUntil(Patience, ok) }
