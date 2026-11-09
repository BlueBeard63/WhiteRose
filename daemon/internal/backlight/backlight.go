// Package backlight reads the screen backlight from sysfs and sets it through
// systemd-logind, which lets the user in the active session change it without
// root (the same path GNOME and KDE use).
package backlight

import (
	"errors"
	"fmt"
	"io/fs"
	"math"
	"sort"
	"strconv"
	"strings"

	"github.com/godbus/dbus/v5"
)

// Sysfs is where the kernel lists backlight devices.
const Sysfs = "/sys/class/backlight"

// Device is one backlight with its current and maximum raw level.
type Device struct {
	Name    string
	Type    string // firmware, platform or raw
	Current int
	Max     int
}

// Percent is the current level as 0-100.
func (d Device) Percent() int {
	if d.Max <= 0 {
		return 0
	}
	return int(math.Round(float64(d.Current) * 100 / float64(d.Max)))
}

// Raw is the raw level for a percentage. Never 0 for a positive request: some
// panels switch off entirely at 0, leaving a black screen.
func (d Device) Raw(percent int) int {
	percent = max(0, min(100, percent))
	raw := int(math.Round(float64(percent) * float64(d.Max) / 100))
	if raw == 0 && percent > 0 {
		raw = 1
	}
	return raw
}

// ErrNone means the machine has no backlight (a desktop with external monitors).
var ErrNone = errors.New("no backlight device")

// typeRank follows the kernel's advice: prefer firmware, then platform, then
// raw interfaces (Documentation/ABI/stable/sysfs-class-backlight).
var typeRank = map[string]int{"firmware": 0, "platform": 1, "raw": 2}

// Find picks the backlight to control from a sysfs tree (fs rooted at
// /sys/class/backlight).
func Find(sys fs.FS) (Device, error) {
	entries, err := fs.ReadDir(sys, ".")
	if err != nil {
		return Device{}, err
	}
	var devs []Device
	for _, e := range entries {
		d, err := read(sys, e.Name())
		if err == nil {
			devs = append(devs, d)
		}
	}
	if len(devs) == 0 {
		return Device{}, ErrNone
	}
	sort.SliceStable(devs, func(i, j int) bool {
		ri, ok := typeRank[devs[i].Type]
		if !ok {
			ri = 3
		}
		rj, ok := typeRank[devs[j].Type]
		if !ok {
			rj = 3
		}
		if ri != rj {
			return ri < rj
		}
		return devs[i].Name < devs[j].Name
	})
	return devs[0], nil
}

func read(sys fs.FS, name string) (Device, error) {
	num := func(file string) (int, error) {
		b, err := fs.ReadFile(sys, name+"/"+file)
		if err != nil {
			return 0, err
		}
		return strconv.Atoi(strings.TrimSpace(string(b)))
	}
	maxv, err := num("max_brightness")
	if err != nil || maxv <= 0 {
		return Device{}, fmt.Errorf("%s: bad max_brightness", name)
	}
	cur, err := num("brightness")
	if err != nil {
		return Device{}, err
	}
	t, _ := fs.ReadFile(sys, name+"/type")
	return Device{Name: name, Type: strings.TrimSpace(string(t)), Current: cur, Max: maxv}, nil
}

// Set changes the backlight through logind's Session.SetBrightness.
func Set(d Device, percent int) error {
	conn, err := dbus.ConnectSystemBus()
	if err != nil {
		return err
	}
	defer conn.Close()
	session := conn.Object("org.freedesktop.login1", "/org/freedesktop/login1/session/auto")
	return session.Call("org.freedesktop.login1.Session.SetBrightness", 0,
		"backlight", d.Name, uint32(d.Raw(percent))).Err
}
