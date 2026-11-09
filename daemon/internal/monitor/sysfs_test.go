package monitor_test

import (
	"testing"
	"testing/fstest"

	"whiterose/daemon/internal/edid/edidtest"
	"whiterose/daemon/internal/monitor"
)

func TestScan(t *testing.T) {
	fsys := fstest.MapFS{
		"card1":                  {Mode: 0o755 | 1<<31}, // the card itself, not a connector
		"renderD128":             {Mode: 0o755 | 1<<31},
		"version":                {Data: []byte("drm 1.1.0\n")},
		"card1-eDP-1/status":     {Data: []byte("connected\n")},
		"card1-eDP-1/edid":       {Data: cheap.Bytes()},
		"card1-DP-1/status":      {Data: []byte("connected\n")},
		"card1-DP-1/edid":        {Data: dellA.Bytes()},
		"card1-DP-2/status":      {Data: []byte("disconnected\n")},
		"card1-DP-2/edid":        {Data: []byte{}},
		"card1-HDMI-A-1/status":  {Data: []byte("connected\n")},
		"card1-HDMI-A-1/edid":    {Data: []byte("garbage")},
		"card1-Writeback-1/dpms": {Data: []byte("On\n")}, // no status file
	}
	got, err := monitor.Scan(fsys)
	if err != nil {
		t.Fatal(err)
	}
	want := []struct {
		connector string
		hasEDID   bool
		mfr       string
	}{
		{"DP-1", true, "DEL"},
		{"HDMI-A-1", false, ""},
		{"eDP-1", true, "HKC"},
	}
	if len(got) != len(want) {
		t.Fatalf("got %d outputs %+v, want %d", len(got), got, len(want))
	}
	for i, w := range want {
		g := got[i]
		if g.Connector != w.connector {
			t.Errorf("[%d] connector %q, want %q", i, g.Connector, w.connector)
		}
		if (g.EDID != nil) != w.hasEDID {
			t.Errorf("[%d] %s: has EDID %v, want %v", i, g.Connector, g.EDID != nil, w.hasEDID)
		}
		if w.hasEDID && g.EDID.Manufacturer != w.mfr {
			t.Errorf("[%d] manufacturer %q, want %q", i, g.EDID.Manufacturer, w.mfr)
		}
	}
}

func TestScanEmpty(t *testing.T) {
	got, err := monitor.Scan(fstest.MapFS{})
	if err != nil {
		t.Fatal(err)
	}
	if len(got) != 0 {
		t.Errorf("got %v, want none", got)
	}
}

func TestScanBuildsUsableIDs(t *testing.T) {
	fsys := fstest.MapFS{
		"card0-DP-3/status": {Data: []byte("connected\n")},
		"card0-DP-3/edid":   {Data: edidtest.Builder{Manufacturer: "LEN", Serial: 99}.Bytes()},
	}
	outs, err := monitor.Scan(fsys)
	if err != nil {
		t.Fatal(err)
	}
	ids := monitor.Identify(outs)
	if len(ids) != 1 || !ids[0].Strong {
		t.Fatalf("got %+v", ids)
	}
}
