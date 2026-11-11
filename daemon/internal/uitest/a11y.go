package uitest

import (
	"fmt"
	"strings"

	"github.com/godbus/dbus/v5"
)

// The session's accessibility bus (AT-SPI): every app that's on shows its
// items there (Qt does with QT_LINUX_ACCESSIBILITY_ALWAYS_ON).
type a11y struct {
	conn *dbus.Conn
}

type ref struct {
	Bus  string
	Path dbus.ObjectPath
}

func (s *Session) access() *a11y {
	if s.a11y != nil {
		return s.a11y
	}
	sess, err := dbus.Dial(s.busAddr)
	if err == nil {
		err = sess.Auth(nil)
	}
	if err == nil {
		err = sess.Hello()
	}
	if err != nil {
		s.t.Fatalf("the session's D-Bus: %v", err)
	}
	defer sess.Close()
	var addr string
	if err := sess.Object("org.a11y.Bus", "/org/a11y/bus").Call("org.a11y.Bus.GetAddress", 0).Store(&addr); err != nil {
		s.t.Fatalf("the session's accessibility bus: %v", err)
	}
	c, err := dbus.Dial(addr)
	if err == nil {
		err = c.Auth(nil)
	}
	if err == nil {
		err = c.Hello()
	}
	if err != nil {
		s.t.Fatalf("the accessibility bus: %v", err)
	}
	s.a11y = &a11y{conn: c}
	return s.a11y
}

// Node is one thing on screen as the accessibility tree has it.
type Node struct {
	Name, Role, Description string
	// Its place in its window: x, y, width, height.
	X, Y, W, H int
	states     []uint32
	ref        ref
}

// AT-SPI's states (the ones the tests look at).
const (
	stateChecked  = 4
	stateEnabled  = 8
	stateFocused  = 12
	stateSelected = 23
	stateShowing  = 25
)

func (n Node) has(state uint) bool {
	i := state / 32
	return int(i) < len(n.states) && n.states[i]&(1<<(state%32)) != 0
}

func (n Node) Checked() bool  { return n.has(stateChecked) }
func (n Node) Selected() bool { return n.has(stateSelected) }
func (n Node) Focused() bool  { return n.has(stateFocused) }
func (n Node) Enabled() bool  { return n.has(stateEnabled) }
func (n Node) Showing() bool  { return n.has(stateShowing) }

func (n Node) String() string {
	s := fmt.Sprintf("%s %q", n.Role, n.Name)
	if n.Description != "" {
		s += fmt.Sprintf(" (%s)", n.Description)
	}
	return s
}

func (a *a11y) node(r ref) Node {
	o := a.conn.Object(r.Bus, r.Path)
	n := Node{ref: r}
	if v, err := o.GetProperty("org.a11y.atspi.Accessible.Name"); err == nil {
		n.Name, _ = v.Value().(string)
	}
	if v, err := o.GetProperty("org.a11y.atspi.Accessible.Description"); err == nil {
		n.Description, _ = v.Value().(string)
	}
	o.Call("org.a11y.atspi.Accessible.GetRoleName", 0).Store(&n.Role)
	o.Call("org.a11y.atspi.Accessible.GetState", 0).Store(&n.states)
	var e struct{ X, Y, W, H int32 }
	// Coordinates relative to its window (1): Wayland has no global ones.
	if o.Call("org.a11y.atspi.Component.GetExtents", 0, uint32(1)).Store(&e) == nil {
		n.X, n.Y, n.W, n.H = int(e.X), int(e.Y), int(e.W), int(e.H)
	}
	return n
}

func (a *a11y) children(r ref) []ref {
	var kids []ref
	a.conn.Object(r.Bus, r.Path).Call("org.a11y.atspi.Accessible.GetChildren", 0).Store(&kids)
	return kids
}

// frames are the top-level windows of every app, by title.
func (a *a11y) frame(title string) (ref, bool) {
	for _, app := range a.children(ref{"org.a11y.atspi.Registry", "/org/a11y/atspi/accessible/root"}) {
		for _, f := range a.children(app) {
			if n := a.node(f); n.Name == title {
				return f, true
			}
		}
	}
	return ref{}, false
}

// apps lists the accessible apps and their windows (for messages).
func (a *a11y) apps() string {
	var out []string
	for _, app := range a.children(ref{"org.a11y.atspi.Registry", "/org/a11y/atspi/accessible/root"}) {
		var frames []string
		for _, f := range a.children(app) {
			frames = append(frames, fmt.Sprintf("%q", a.node(f).Name))
		}
		out = append(out, a.node(app).Name+"["+strings.Join(frames, ", ")+"]")
	}
	return strings.Join(out, " ")
}

// all the nodes under r that are on screen, in tree order.
func (a *a11y) all(r ref) []Node {
	var out []Node
	var walk func(ref, int)
	walk = func(r ref, depth int) {
		if depth > 40 {
			return
		}
		for _, k := range a.children(r) {
			n := a.node(k)
			if len(n.states) > 0 && !n.Showing() {
				continue
			}
			out = append(out, n)
			walk(k, depth+1)
		}
	}
	walk(r, 0)
	return out
}

// text is what an editable field holds.
func (a *a11y) text(n Node) string {
	var s string
	a.conn.Object(n.ref.Bus, n.ref.Path).Call("org.a11y.atspi.Text.GetText", 0, int32(0), int32(-1)).Store(&s)
	return s
}

// ---- finding things -------------------------------------------------------------------

// Query describes what to find: by name, and optionally its role ("push
// button", "list item", "page tab", "check box", "text", "combo box",
// "radio button", "menu item") and description.
type Query struct {
	name, role, desc string
	hasDesc          bool
}

// Named finds things with this accessible name (a button's label, a
// file's name, a field's label or placeholder).
func Named(name string) Query { return Query{name: name} }

// Role narrows a query to a role ("push button", "list item"…).
func (q Query) Role(role string) Query { q.role = role; return q }

// Described narrows a query to things whose description is exactly d (a
// file being renamed: "→ new name").
func (q Query) Described(d string) Query { q.desc, q.hasDesc = d, true; return q }

func (q Query) matches(n Node) bool {
	return n.Name == q.name && (q.role == "" || n.Role == q.role) && (!q.hasDesc || n.Description == q.desc)
}

func (q Query) String() string {
	s := fmt.Sprintf("%q", q.name)
	if q.role != "" {
		s = q.role + " " + s
	}
	if q.hasDesc {
		s += fmt.Sprintf(" described %q", q.desc)
	}
	return s
}

func describe(nodes []Node) string {
	var b strings.Builder
	for _, n := range nodes {
		if n.Name == "" && n.Description == "" {
			continue
		}
		b.WriteString("  " + n.String() + "\n")
	}
	return b.String()
}
