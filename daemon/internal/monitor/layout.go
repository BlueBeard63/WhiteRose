package monitor

import (
	"errors"
	"fmt"
	"math"
	"sort"
)

// Rect is a monitor's area in the global logical coordinate space.
type Rect struct {
	X, Y, W, H int
}

func (r Rect) right() int  { return r.X + r.W }
func (r Rect) bottom() int { return r.Y + r.H }

func (r Rect) overlaps(o Rect) bool {
	return r.X < o.right() && o.X < r.right() && r.Y < o.bottom() && o.Y < r.bottom()
}

// touches reports whether r and o share an edge segment of positive length.
// Corner-only contact does not count: the pointer could not cross it.
func (r Rect) touches(o Rect) bool {
	vOverlap := min(r.bottom(), o.bottom()) - max(r.Y, o.Y)
	hOverlap := min(r.right(), o.right()) - max(r.X, o.X)
	sideBySide := (r.right() == o.X || o.right() == r.X) && vOverlap > 0
	stacked := (r.bottom() == o.Y || o.bottom() == r.Y) && hOverlap > 0
	return sideBySide || stacked
}

// Transform is an output rotation/flip, as in the Wayland protocol.
type Transform string

const (
	TransformNormal     Transform = "normal"
	Transform90         Transform = "90"
	Transform180        Transform = "180"
	Transform270        Transform = "270"
	TransformFlipped    Transform = "flipped"
	TransformFlipped90  Transform = "flipped-90"
	TransformFlipped180 Transform = "flipped-180"
	TransformFlipped270 Transform = "flipped-270"
)

// Valid reports whether t is a known transform. The empty string means normal.
func (t Transform) Valid() bool {
	switch t {
	case "", TransformNormal, Transform90, Transform180, Transform270,
		TransformFlipped, TransformFlipped90, TransformFlipped180, TransformFlipped270:
		return true
	}
	return false
}

func (t Transform) swapsAxes() bool {
	switch t {
	case Transform90, Transform270, TransformFlipped90, TransformFlipped270:
		return true
	}
	return false
}

// LogicalSize returns the size an output occupies in the global space: its
// mode divided by its scale, with axes swapped for 90°/270° rotations.
// Rounding matches what Wayland compositors do for fractional scales.
func LogicalSize(modeW, modeH int, scale float64, t Transform) (w, h int) {
	if scale <= 0 {
		scale = 1
	}
	w = int(math.Round(float64(modeW) / scale))
	h = int(math.Round(float64(modeH) / scale))
	if t.swapsAxes() {
		w, h = h, w
	}
	return w, h
}

// Layout errors.
var (
	ErrOverlap      = errors.New("monitors overlap")
	ErrDisconnected = errors.New("monitors are not all connected")
	ErrEmptyRect    = errors.New("monitor has zero size")
)

// ValidateLayout checks that no two monitors overlap and that every monitor
// shares an edge with the rest (so the pointer can reach each one).
// Keys are monitor IDs.
func ValidateLayout(rects map[string]Rect) error {
	ids := sortedKeys(rects)
	var errs []error
	for _, id := range ids {
		if r := rects[id]; r.W <= 0 || r.H <= 0 {
			errs = append(errs, fmt.Errorf("%s: %w", id, ErrEmptyRect))
		}
	}
	for i, a := range ids {
		for _, b := range ids[i+1:] {
			if rects[a].overlaps(rects[b]) {
				errs = append(errs, fmt.Errorf("%s and %s: %w", a, b, ErrOverlap))
			}
		}
	}
	if len(errs) > 0 {
		return errors.Join(errs...)
	}
	if len(ids) < 2 {
		return nil
	}

	// Flood fill from the first monitor along shared edges.
	seen := map[string]bool{ids[0]: true}
	queue := []string{ids[0]}
	for len(queue) > 0 {
		cur := queue[0]
		queue = queue[1:]
		for _, id := range ids {
			if !seen[id] && rects[cur].touches(rects[id]) {
				seen[id] = true
				queue = append(queue, id)
			}
		}
	}
	for _, id := range ids {
		if !seen[id] {
			errs = append(errs, fmt.Errorf("%s: %w", id, ErrDisconnected))
		}
	}
	return errors.Join(errs...)
}

// Normalize shifts the layout so its top-left-most corner is at (0, 0),
// which keeps saved positions positive and stable after a drag.
func Normalize(rects map[string]Rect) map[string]Rect {
	if len(rects) == 0 {
		return rects
	}
	minX, minY := math.MaxInt, math.MaxInt
	for _, r := range rects {
		minX, minY = min(minX, r.X), min(minY, r.Y)
	}
	out := make(map[string]Rect, len(rects))
	for id, r := range rects {
		r.X -= minX
		r.Y -= minY
		out[id] = r
	}
	return out
}

// Size is a logical width and height.
type Size struct{ W, H int }

// AutoLayout places monitors left to right in the given order, aligned to
// the top. It is the fallback when no profile matches.
func AutoLayout(order []string, sizes map[string]Size) map[string]Rect {
	out := make(map[string]Rect, len(order))
	x := 0
	for _, id := range order {
		s := sizes[id]
		out[id] = Rect{X: x, Y: 0, W: s.W, H: s.H}
		x += s.W
	}
	return out
}

func sortedKeys[V any](m map[string]V) []string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	return keys
}
