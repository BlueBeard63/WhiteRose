package monitor_test

import (
	"encoding/hex"
	"encoding/json"
	"os"
	"testing"

	"whiterose/daemon/internal/edid"
	"whiterose/daemon/internal/monitor"
)

// The compositor computes the same IDs in Rust (compositor/src/monitor_id.rs)
// and checks the same file: a monitor's saved layout must match whichever
// side looks it up.
func TestSharedFixture(t *testing.T) {
	b, err := os.ReadFile("../../../testdata/monitor-ids.json")
	if err != nil {
		t.Fatal(err)
	}
	var cases []struct {
		Name    string
		Outputs []struct{ Connector, Path, EDID string }
		Want    []struct {
			Connector, ID string
			Strong        bool
		}
	}
	if err := json.Unmarshal(b, &cases); err != nil {
		t.Fatal(err)
	}
	for _, c := range cases {
		var outs []monitor.Output
		for _, o := range c.Outputs {
			m := monitor.Output{Connector: o.Connector, Path: o.Path}
			if o.EDID != "" {
				raw, _ := hex.DecodeString(o.EDID)
				m.EDID, _ = edid.Parse(raw)
			}
			outs = append(outs, m)
		}
		got := monitor.Identify(outs)
		for i, w := range c.Want {
			if got[i].Connector != w.Connector || got[i].ID != w.ID || got[i].Strong != w.Strong {
				t.Errorf("%s: %s got %s/%v, want %s/%v", c.Name, w.Connector, got[i].ID, got[i].Strong, w.ID, w.Strong)
			}
		}
	}
}
