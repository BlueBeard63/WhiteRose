package uitest

import (
	"strings"
	"time"
)

// The pointer and keyboard, driven through the compositor exactly as real
// ones are (its debug input, on in test sessions only).

func (s *Session) pointer(x, y float64, button string) {
	p := map[string]any{"x": x, "y": y}
	if button != "" {
		p["button"] = button
	}
	s.send(map[string]any{"debug_pointer": p})
	time.Sleep(40 * time.Millisecond)
}

// scroll turns the wheel over x, y: clicks down (negative: up).
func (s *Session) scroll(x, y float64, clicks int) {
	s.send(map[string]any{"debug_pointer": map[string]any{"x": x, "y": y, "scroll": clicks}})
	time.Sleep(120 * time.Millisecond)
}

func (s *Session) key(code int, down bool) {
	// xkb keycodes are evdev's + 8.
	s.send(map[string]any{"debug_key": map[string]any{"code": code + 8, "down": down}})
	time.Sleep(15 * time.Millisecond)
}

// evdev codes of a US keyboard: the character, and whether it needs Shift.
var keys = func() map[rune][2]int {
	m := map[rune][2]int{' ': {57, 0}, '\n': {28, 0}, '\t': {15, 0}}
	rows := []struct {
		plain, shifted string
		start          int
	}{
		{"1234567890-=", "!@#$%^&*()_+", 2},
		{"qwertyuiop[]", "QWERTYUIOP{}", 16},
		{"asdfghjkl;'`", "ASDFGHJKL:\"~", 30},
		{"\\zxcvbnm,./", "|ZXCVBNM<>?", 43},
	}
	for _, r := range rows {
		sh := []rune(r.shifted)
		for i, c := range []rune(r.plain) {
			m[c] = [2]int{r.start + i, 0}
			m[sh[i]] = [2]int{r.start + i, 1}
		}
	}
	return m
}()

var named = map[string]int{
	"escape": 1, "esc": 1, "backspace": 14, "tab": 15, "return": 28, "enter": 28, "space": 57,
	"ctrl": 29, "shift": 42, "alt": 56, "super": 125,
	"f1": 59, "f2": 60, "f3": 61, "f4": 62, "f5": 63, "f6": 64, "f7": 65, "f8": 66, "f9": 67, "f10": 68, "f11": 87, "f12": 88,
	"home": 102, "up": 103, "pageup": 104, "left": 105, "right": 106, "end": 107, "down": 108, "pagedown": 109,
	"insert": 110, "delete": 111, "del": 111, "print": 99,
}

const shiftKey = 42

func (s *Session) typeText(text string) {
	s.t.Helper()
	for _, c := range text {
		k, ok := keys[c]
		if !ok {
			s.t.Fatalf("can't type %q on a US keyboard", c)
		}
		if k[1] == 1 {
			s.key(shiftKey, true)
		}
		s.key(k[0], true)
		s.key(k[0], false)
		if k[1] == 1 {
			s.key(shiftKey, false)
		}
	}
	time.Sleep(100 * time.Millisecond)
}

// Press presses a key combination ("super+q") with no window in mind.
func (s *Session) Press(combo string) { s.t.Helper(); s.press(combo) }

func (s *Session) press(combo string) {
	s.t.Helper()
	var codes []int
	for _, part := range strings.Split(combo, "+") {
		p := strings.ToLower(strings.TrimSpace(part))
		if c, ok := named[p]; ok {
			codes = append(codes, c)
		} else if k, ok := keys[[]rune(p)[0]]; ok && len([]rune(p)) == 1 {
			codes = append(codes, k[0])
		} else {
			s.t.Fatalf("no key %q in %q", part, combo)
		}
	}
	for _, c := range codes {
		s.key(c, true)
	}
	for i := len(codes) - 1; i >= 0; i-- {
		s.key(codes[i], false)
	}
	time.Sleep(150 * time.Millisecond)
}

// Drag presses the left button at (x1, y1), moves to (x2, y2) in steps and
// lets go, with the keys `hold` ("super", "shift") held throughout.
func (s *Session) Drag(x1, y1, x2, y2 float64, hold ...string) {
	s.t.Helper()
	var codes []int
	for _, h := range hold {
		c, ok := named[strings.ToLower(h)]
		if !ok {
			s.t.Fatalf("no key %q to hold", h)
		}
		codes = append(codes, c)
	}
	for _, c := range codes {
		s.key(c, true)
	}
	s.pointer(x1, y1, "")
	s.pointer(x1, y1, "press")
	const steps = 8
	for i := 1; i <= steps; i++ {
		f := float64(i) / steps
		s.pointer(x1+(x2-x1)*f, y1+(y2-y1)*f, "")
	}
	s.pointer(x2, y2, "release")
	for i := len(codes) - 1; i >= 0; i-- {
		s.key(codes[i], false)
	}
	time.Sleep(150 * time.Millisecond)
}
