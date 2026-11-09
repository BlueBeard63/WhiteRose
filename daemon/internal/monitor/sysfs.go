package monitor

import (
	"errors"
	"fmt"
	"io/fs"
	"regexp"
	"sort"
	"strings"

	"whiterose/daemon/internal/edid"
)

// SysfsDRM is where the kernel exposes DRM connectors.
const SysfsDRM = "/sys/class/drm"

var connectorDir = regexp.MustCompile(`^card[0-9]+-(.+)$`)

// Scan reads connected outputs from a sysfs DRM tree (normally
// os.DirFS(SysfsDRM)). Taking an fs.FS keeps it testable.
//
// Sysfs does not expose the DRM PATH property, so Output.Path is left empty;
// the compositor fills it in when it reports outputs over IPC.
//
// A connector with unreadable or invalid EDID is still returned, with a nil
// EDID, so the user can see and configure it.
func Scan(fsys fs.FS) ([]Output, error) {
	entries, err := fs.ReadDir(fsys, ".")
	if err != nil {
		return nil, fmt.Errorf("scan drm: %w", err)
	}
	var outs []Output
	for _, e := range entries {
		m := connectorDir.FindStringSubmatch(e.Name())
		if m == nil {
			continue
		}
		status, err := fs.ReadFile(fsys, e.Name()+"/status")
		if err != nil {
			if errors.Is(err, fs.ErrNotExist) {
				continue
			}
			return nil, fmt.Errorf("scan drm %s: %w", e.Name(), err)
		}
		if strings.TrimSpace(string(status)) != "connected" {
			continue
		}
		o := Output{Connector: m[1]}
		if raw, err := fs.ReadFile(fsys, e.Name()+"/edid"); err == nil {
			if info, err := edid.Parse(raw); err == nil {
				o.EDID = info
			}
		}
		outs = append(outs, o)
	}
	sort.Slice(outs, func(i, j int) bool { return outs[i].Connector < outs[j].Connector })
	return outs, nil
}
