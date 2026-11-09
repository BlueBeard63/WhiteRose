// Package monitor gives connected monitors stable IDs and validates layouts.
//
// Connector names (DP-1, HDMI-A-2) describe a port, not a monitor, and change
// with cables, docks and probe order. WhiteRose identifies monitors by the
// identity fields of their EDID instead. See ARCHITECTURE.md.
package monitor

import (
	"crypto/sha256"
	"encoding/hex"
	"sort"
	"strconv"
	"strings"

	"whiterose/daemon/internal/edid"
)

// BuiltinID is the ID of the single built-in panel (eDP, LVDS, DSI). A laptop
// panel never moves between ports, and keeping its ID fixed means a panel
// replacement does not orphan its config.
const BuiltinID = "builtin"

// Output is a connected display as seen by the scanner or the compositor.
type Output struct {
	// Connector is the kernel connector name, e.g. "DP-1" or "eDP-1".
	Connector string
	// Path is the DRM PATH property for MST (dock / daisy-chain) outputs,
	// e.g. "mst:62-1-8". Empty for directly attached outputs.
	Path string
	// EDID is nil when the monitor provided no readable EDID.
	EDID *edid.Info
}

// Builtin reports whether the connector is an internal panel.
func (o Output) Builtin() bool {
	for _, p := range []string{"eDP", "LVDS", "DSI"} {
		if strings.HasPrefix(o.Connector, p) {
			return true
		}
	}
	return false
}

// Identified is an Output with its stable ID.
type Identified struct {
	Output
	ID string
	// Strong is true when the ID comes from a real serial number and is
	// unique among connected monitors. Weak IDs can change if identical
	// monitors are swapped between ports; the settings UI should ask the user
	// to confirm which is which.
	Strong bool
}

// Fingerprint hashes the identity fields of an EDID. It deliberately ignores
// timing and extension data, which some monitors change when FreeSync, HDR
// or input settings are toggled.
//
// Changing this function changes every user's monitor IDs. It is pinned by a
// golden test.
func Fingerprint(e *edid.Info) string {
	fields := []string{
		e.Manufacturer,
		strconv.Itoa(int(e.ProductCode)),
		strconv.FormatUint(uint64(e.Serial), 10),
		e.SerialString,
		strconv.Itoa(e.Week),
		strconv.Itoa(e.Year),
		e.Name,
	}
	sum := sha256.Sum256([]byte(strings.Join(fields, "\x1f")))
	return "edid:" + hex.EncodeToString(sum[:])[:12]
}

// HasSerial reports whether the EDID carries a usable serial number. Zero and
// a few common filler values are treated as missing.
func HasSerial(e *edid.Info) bool {
	if e.SerialString != "" {
		return true
	}
	switch e.Serial {
	case 0, 0x01010101, 0xFFFFFFFF:
		return false
	}
	return true
}

// Identify assigns stable IDs to connected outputs. The result is sorted by
// connector so it is deterministic.
//
// Rules:
//   - exactly one built-in panel gets BuiltinID;
//   - a monitor with a usable serial whose fingerprint is unique gets that
//     fingerprint as a strong ID;
//   - a monitor without a serial that is the only one with its fingerprint
//     gets the fingerprint as a weak ID (so moving ports keeps its config);
//   - monitors sharing a fingerprint get "<fingerprint>@<location>", where
//     location is the MST path if known, otherwise the connector;
//   - a monitor with no EDID gets "connector:<name>".
func Identify(outputs []Output) []Identified {
	outs := append([]Output(nil), outputs...)
	sort.Slice(outs, func(i, j int) bool { return outs[i].Connector < outs[j].Connector })

	builtins := 0
	counts := map[string]int{}
	for _, o := range outs {
		if o.Builtin() {
			builtins++
		}
		if o.EDID != nil {
			counts[Fingerprint(o.EDID)]++
		}
	}

	res := make([]Identified, 0, len(outs))
	for _, o := range outs {
		id := Identified{Output: o}
		switch {
		case o.Builtin() && builtins == 1:
			id.ID, id.Strong = BuiltinID, true
		case o.EDID == nil:
			id.ID = "connector:" + o.Connector
		default:
			fp := Fingerprint(o.EDID)
			if counts[fp] > 1 {
				id.ID = fp + "@" + location(o)
			} else {
				id.ID, id.Strong = fp, HasSerial(o.EDID)
			}
		}
		res = append(res, id)
	}
	return res
}

func location(o Output) string {
	if o.Path != "" {
		return o.Path
	}
	return o.Connector
}
