package monitor_test

import (
	"errors"
	"reflect"
	"testing"

	"whiterose/daemon/internal/monitor"
)

func TestLogicalSize(t *testing.T) {
	tests := []struct {
		w, h   int
		scale  float64
		tr     monitor.Transform
		ww, wh int
	}{
		{2560, 1440, 1, "", 2560, 1440},
		{2560, 1440, 1.25, monitor.TransformNormal, 2048, 1152},
		{2880, 1800, 1.5, "", 1920, 1200},
		{3840, 2160, 2, monitor.Transform90, 1080, 1920},
		{1920, 1080, 1, monitor.TransformFlipped270, 1080, 1920},
		{1920, 1080, 0, "", 1920, 1080}, // unset scale means 1
		{2560, 1600, 1.6, "", 1600, 1000},
	}
	for _, tt := range tests {
		gw, gh := monitor.LogicalSize(tt.w, tt.h, tt.scale, tt.tr)
		if gw != tt.ww || gh != tt.wh {
			t.Errorf("LogicalSize(%d,%d,%g,%q) = %dx%d, want %dx%d", tt.w, tt.h, tt.scale, tt.tr, gw, gh, tt.ww, tt.wh)
		}
	}
}

func TestTransformValid(t *testing.T) {
	for _, ok := range []monitor.Transform{"", "normal", "90", "180", "270", "flipped", "flipped-90", "flipped-180", "flipped-270"} {
		if !ok.Valid() {
			t.Errorf("%q should be valid", ok)
		}
	}
	for _, bad := range []monitor.Transform{"45", "upside-down", "Normal"} {
		if bad.Valid() {
			t.Errorf("%q should be invalid", bad)
		}
	}
}

func TestValidateLayout(t *testing.T) {
	builtin := monitor.Rect{X: 0, Y: 288, W: 1600, H: 1000}
	left := monitor.Rect{X: 1600, Y: 0, W: 2048, H: 1152}
	right := monitor.Rect{X: 3648, Y: 0, W: 2048, H: 1152}

	tests := []struct {
		name  string
		rects map[string]monitor.Rect
		want  []error
	}{
		{"empty", map[string]monitor.Rect{}, nil},
		{"single", map[string]monitor.Rect{"a": left}, nil},
		{"desk: laptop + two side by side", map[string]monitor.Rect{"builtin": builtin, "l": left, "r": right}, nil},
		{"stacked vertically", map[string]monitor.Rect{"a": {0, 0, 1920, 1080}, "b": {0, 1080, 1920, 1080}}, nil},
		{"partial edge contact counts", map[string]monitor.Rect{"a": {0, 0, 1920, 1080}, "b": {1920, 1000, 1920, 1080}}, nil},
		{"overlap", map[string]monitor.Rect{"a": {0, 0, 1920, 1080}, "b": {1900, 0, 1920, 1080}}, []error{monitor.ErrOverlap}},
		{"gap", map[string]monitor.Rect{"a": {0, 0, 1920, 1080}, "b": {1921, 0, 1920, 1080}}, []error{monitor.ErrDisconnected}},
		{"corner only", map[string]monitor.Rect{"a": {0, 0, 1920, 1080}, "b": {1920, 1080, 1920, 1080}}, []error{monitor.ErrDisconnected}},
		{"zero size", map[string]monitor.Rect{"a": {0, 0, 0, 1080}}, []error{monitor.ErrEmptyRect}},
		{
			"chain through middle monitor is connected",
			map[string]monitor.Rect{"a": {0, 0, 100, 100}, "b": {100, 0, 100, 100}, "c": {200, 0, 100, 100}},
			nil,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := monitor.ValidateLayout(tt.rects)
			if len(tt.want) == 0 {
				if err != nil {
					t.Fatalf("unexpected error: %v", err)
				}
				return
			}
			for _, w := range tt.want {
				if !errors.Is(err, w) {
					t.Errorf("err = %v, want %v", err, w)
				}
			}
		})
	}
}

func TestNormalize(t *testing.T) {
	in := map[string]monitor.Rect{
		"a": {X: -24, Y: -50, W: 100, H: 100},
		"b": {X: 76, Y: 0, W: 100, H: 100},
	}
	want := map[string]monitor.Rect{
		"a": {X: 0, Y: 0, W: 100, H: 100},
		"b": {X: 100, Y: 50, W: 100, H: 100},
	}
	got := monitor.Normalize(in)
	if !reflect.DeepEqual(got, want) {
		t.Errorf("got %v, want %v", got, want)
	}
	if in["a"].X != -24 {
		t.Error("Normalize mutated its input")
	}
	if err := monitor.ValidateLayout(got); err != nil {
		t.Errorf("normalized layout invalid: %v", err)
	}
}

func TestAutoLayoutIsValid(t *testing.T) {
	sizes := map[string]monitor.Size{"builtin": {1600, 1000}, "x": {2048, 1152}, "y": {1080, 1920}}
	got := monitor.AutoLayout([]string{"builtin", "x", "y"}, sizes)
	want := map[string]monitor.Rect{
		"builtin": {0, 0, 1600, 1000},
		"x":       {1600, 0, 2048, 1152},
		"y":       {3648, 0, 1080, 1920},
	}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("got %v, want %v", got, want)
	}
	if err := monitor.ValidateLayout(got); err != nil {
		t.Errorf("auto layout invalid: %v", err)
	}
}
