// Package power reads battery details UPower has but QuickShell doesn't
// show: charge cycles, the charge history behind the Settings chart, and the
// charge limit (on batteries that support one), through UPower's D-Bus API.
package power

import (
	"errors"
	"sort"
	"strings"

	"github.com/godbus/dbus/v5"
)

const (
	upower    = "org.freedesktop.UPower"
	devIface  = "org.freedesktop.UPower.Device"
	typeBatt  = 2 // UPower device type: battery
	stateChrg = 1 // charging
	stateFull = 4
)

// Point is one history sample.
type Point struct {
	Time     uint32  `json:"time"` // unix seconds
	Percent  float64 `json:"percent"`
	Charging bool    `json:"charging"`
}

// Battery is what the Power & battery page shows beyond QuickShell's data.
type Battery struct {
	Model           string  `json:"model"`
	Cycles          int32   `json:"cycles"` // -1 when unknown
	LimitSupported  bool    `json:"limit_supported"`
	LimitEnabled    bool    `json:"limit_enabled"`
	LimitStart      uint32  `json:"limit_start"`
	LimitEnd        uint32  `json:"limit_end"`
	History         []Point `json:"history"`
	EnergyFull      float64 `json:"energy_full"`
	EnergyFullDsgn  float64 `json:"energy_full_design"`
	HealthPercent   float64 `json:"health"`
	HistoryInterval int     `json:"history_interval"` // seconds per bar
}

// ErrNoBattery: a desktop.
var ErrNoBattery = errors.New("no battery")

func find(conn *dbus.Conn) (dbus.BusObject, error) {
	var devs []dbus.ObjectPath
	if err := conn.Object(upower, "/org/freedesktop/UPower").Call(upower+".EnumerateDevices", 0).Store(&devs); err != nil {
		return nil, err
	}
	for _, p := range devs {
		o := conn.Object(upower, p)
		t, err := o.GetProperty(devIface + ".Type")
		if err != nil {
			continue
		}
		if v, _ := t.Value().(uint32); v == typeBatt {
			if ps, err := o.GetProperty(devIface + ".PowerSupply"); err == nil {
				if b, _ := ps.Value().(bool); b {
					return o, nil
				}
			}
		}
	}
	return nil, ErrNoBattery
}

func prop[T any](o dbus.BusObject, name string, def T) T {
	v, err := o.GetProperty(devIface + "." + name)
	if err != nil {
		return def
	}
	if x, ok := v.Value().(T); ok {
		return x
	}
	return def
}

// Bars turns UPower's charge history (newest first, irregular) into `bars`
// evenly spaced samples ending `now`, oldest first: each the last known
// value at that time.
func Bars(raw []Point, now uint32, span uint32, bars int) []Point {
	sort.Slice(raw, func(i, j int) bool { return raw[i].Time < raw[j].Time })
	out := make([]Point, 0, bars)
	step := span / uint32(bars)
	j := -1
	for i := 0; i < bars; i++ {
		t := now - span + step*uint32(i+1)
		for j+1 < len(raw) && raw[j+1].Time <= t {
			j++
		}
		if j < 0 {
			out = append(out, Point{Time: t, Percent: -1}) // no data yet
			continue
		}
		out = append(out, Point{Time: t, Percent: raw[j].Percent, Charging: raw[j].Charging})
	}
	return out
}

// Read gets the battery details and the last 24 hours in 30 bars.
func Read(conn *dbus.Conn, now uint32) (Battery, error) {
	o, err := find(conn)
	if err != nil {
		return Battery{}, err
	}
	b := Battery{
		Model:          strings.TrimSpace(prop(o, "Model", "")),
		Cycles:         prop(o, "ChargeCycles", int32(-1)),
		LimitSupported: prop(o, "ChargeThresholdSupported", false),
		LimitEnabled:   prop(o, "ChargeThresholdEnabled", false),
		LimitStart:     prop(o, "ChargeStartThreshold", uint32(0)),
		LimitEnd:       prop(o, "ChargeEndThreshold", uint32(0)),
		EnergyFull:     prop(o, "EnergyFull", 0.0),
		EnergyFullDsgn: prop(o, "EnergyFullDesign", 0.0),
		HealthPercent:  prop(o, "Capacity", 0.0),
	}
	var raw []struct {
		Time  uint32
		Value float64
		State uint32
	}
	const span, bars = 24 * 3600, 30
	if err := o.Call(devIface+".GetHistory", 0, "charge", uint32(span), uint32(200)).Store(&raw); err == nil {
		pts := make([]Point, len(raw))
		for i, r := range raw {
			pts[i] = Point{Time: r.Time, Percent: r.Value, Charging: r.State == stateChrg || r.State == stateFull}
		}
		b.History = Bars(pts, now, span, bars)
		b.HistoryInterval = span / bars
	}
	return b, nil
}

// SetLimit turns the charge limit on or off (UPower applies the
// thresholds; polkit allows the active session's user).
func SetLimit(conn *dbus.Conn, on bool) error {
	o, err := find(conn)
	if err != nil {
		return err
	}
	if !prop(o, "ChargeThresholdSupported", false) {
		return errors.New("this battery has no charge limit")
	}
	return o.Call(devIface+".EnableChargeThreshold", 0, on).Err
}
