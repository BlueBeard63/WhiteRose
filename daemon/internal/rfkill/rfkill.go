// Package rfkill reads and sets the kernel's radio kill switches: airplane
// mode is every radio (Wi-Fi, Bluetooth, mobile broadband) soft-blocked. The
// active session's user can write /dev/rfkill (logind grants it), so no root.
package rfkill

import (
	"encoding/binary"
	"io"
	"io/fs"
	"os"
	"strings"
)

const (
	Sysfs  = "/sys/class/rfkill"
	Device = "/dev/rfkill"
)

// Radio is one kill switch.
type Radio struct {
	Name string
	Type string // wlan, bluetooth, wwan, ...
	Soft bool   // blocked by software (what airplane mode sets)
	Hard bool   // blocked by a hardware switch (can't be changed here)
}

// Scan lists the radios from a sysfs tree (fs rooted at /sys/class/rfkill).
func Scan(sys fs.FS) ([]Radio, error) {
	entries, err := fs.ReadDir(sys, ".")
	if err != nil {
		return nil, err
	}
	var out []Radio
	for _, e := range entries {
		get := func(f string) string {
			b, _ := fs.ReadFile(sys, e.Name()+"/"+f)
			return strings.TrimSpace(string(b))
		}
		if get("type") == "" {
			continue
		}
		out = append(out, Radio{Name: get("name"), Type: get("type"), Soft: get("soft") == "1", Hard: get("hard") == "1"})
	}
	return out, nil
}

// Airplane is true when there are radios and every one is blocked.
func Airplane(radios []Radio) bool {
	for _, r := range radios {
		if !r.Soft && !r.Hard {
			return false
		}
	}
	return len(radios) > 0
}

// Kernel ABI (include/uapi/linux/rfkill.h): struct rfkill_event.
const (
	typeAll   = 0
	opChange  = 2
	opChgAll  = 3
	eventSize = 8
)

// Event is one rfkill_event read from /dev/rfkill.
type Event struct {
	Idx  uint32
	Type uint8
	Op   uint8
	Soft uint8
	Hard uint8
}

// SetAllEvent is the event that soft-blocks (or unblocks) every radio.
func SetAllEvent(blocked bool) []byte {
	b := make([]byte, eventSize)
	binary.NativeEndian.PutUint32(b[0:], 0)
	b[4] = typeAll
	b[5] = opChgAll
	if blocked {
		b[6] = 1
	}
	return b
}

// SetAll soft-blocks or unblocks every radio.
func SetAll(blocked bool) error {
	f, err := os.OpenFile(Device, os.O_WRONLY, 0)
	if err != nil {
		return err
	}
	defer f.Close()
	_, err = f.Write(SetAllEvent(blocked))
	return err
}

// ReadEvents reads events until r fails, calling fn for each. Reading
// /dev/rfkill blocks until something changes, so this costs nothing at idle.
// Newer kernels send larger events; the extra bytes are skipped.
func ReadEvents(r io.Reader, fn func(Event)) error {
	buf := make([]byte, 64)
	for {
		n, err := r.Read(buf)
		if err != nil {
			return err
		}
		if n < eventSize {
			continue
		}
		fn(Event{Idx: binary.NativeEndian.Uint32(buf[0:]), Type: buf[4], Op: buf[5], Soft: buf[6], Hard: buf[7]})
	}
}
