package monitor_test

import (
	"strings"
	"testing"

	"whiterose/daemon/internal/edid"
	"whiterose/daemon/internal/edid/edidtest"
	"whiterose/daemon/internal/monitor"
)

func info(t *testing.T, b edidtest.Builder) *edid.Info {
	t.Helper()
	e, err := edid.Parse(b.Bytes())
	if err != nil {
		t.Fatal(err)
	}
	return e
}

var (
	dellA = edidtest.Builder{Manufacturer: "DEL", ProductCode: 0x42AC, Serial: 0x30335A4C, Week: 12, Year: 2023, Name: "DELL U2723QE", SerialString: "7F3K2Q3"}
	dellB = edidtest.Builder{Manufacturer: "DEL", ProductCode: 0x42AC, Serial: 0x30335A4D, Week: 12, Year: 2023, Name: "DELL U2723QE", SerialString: "7F3K2Q4"}
	cheap = edidtest.Builder{Manufacturer: "HKC", ProductCode: 0x2413, Year: 2022, Name: "24N1"} // no serial at all
)

// TestFingerprintGolden pins the ID format. If this fails, every user's saved
// monitor config would stop matching: change it only with a migration.
func TestFingerprintGolden(t *testing.T) {
	const want = "edid:0d4384bf042b"
	if got := monitor.Fingerprint(info(t, dellA)); got != want {
		t.Fatalf("Fingerprint changed: got %s, want %s", got, want)
	}
}

func TestFingerprintIgnoresNonIdentityData(t *testing.T) {
	a := dellA
	b := dellA
	b.Extensions = 3     // HDR / FreeSync toggles often change extensions
	b.BadChecksum = true // and occasionally ship a broken checksum
	if monitor.Fingerprint(info(t, a)) != monitor.Fingerprint(info(t, b)) {
		t.Error("fingerprint must only depend on identity fields")
	}
}

func TestFingerprintDistinguishesSerials(t *testing.T) {
	if monitor.Fingerprint(info(t, dellA)) == monitor.Fingerprint(info(t, dellB)) {
		t.Error("different serials produced the same fingerprint")
	}
}

func TestHasSerial(t *testing.T) {
	tests := []struct {
		name string
		b    edidtest.Builder
		want bool
	}{
		{"serial string", edidtest.Builder{SerialString: "X1"}, true},
		{"numeric serial", edidtest.Builder{Serial: 12345}, true},
		{"zero", edidtest.Builder{}, false},
		{"0x01010101 filler", edidtest.Builder{Serial: 0x01010101}, false},
		{"0xFFFFFFFF filler", edidtest.Builder{Serial: 0xFFFFFFFF}, false},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := monitor.HasSerial(info(t, tt.b)); got != tt.want {
				t.Errorf("HasSerial = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestIdentify(t *testing.T) {
	fpA := monitor.Fingerprint(info(t, dellA))
	fpB := monitor.Fingerprint(info(t, dellB))
	fpCheap := monitor.Fingerprint(info(t, cheap))

	type want struct {
		connector, id string
		strong        bool
	}
	tests := []struct {
		name string
		in   []monitor.Output
		want []want
	}{
		{
			name: "laptop on its own",
			in:   []monitor.Output{{Connector: "eDP-1", EDID: info(t, cheap)}},
			want: []want{{"eDP-1", monitor.BuiltinID, true}},
		},
		{
			name: "two identical models with distinct serials",
			in: []monitor.Output{
				{Connector: "DP-2", EDID: info(t, dellB)},
				{Connector: "DP-1", EDID: info(t, dellA)},
				{Connector: "eDP-1", EDID: info(t, cheap)},
			},
			want: []want{{"DP-1", fpA, true}, {"DP-2", fpB, true}, {"eDP-1", monitor.BuiltinID, true}},
		},
		{
			name: "single monitor without serial keeps a portable weak id",
			in:   []monitor.Output{{Connector: "HDMI-A-1", EDID: info(t, cheap)}},
			want: []want{{"HDMI-A-1", fpCheap, false}},
		},
		{
			name: "identical monitors without serials are told apart by port",
			in: []monitor.Output{
				{Connector: "DP-1", EDID: info(t, cheap)},
				{Connector: "HDMI-A-1", EDID: info(t, cheap)},
			},
			want: []want{{"DP-1", fpCheap + "@DP-1", false}, {"HDMI-A-1", fpCheap + "@HDMI-A-1", false}},
		},
		{
			name: "MST path is preferred over connector for collisions",
			in: []monitor.Output{
				{Connector: "DP-5", Path: "mst:62-1-8", EDID: info(t, cheap)},
				{Connector: "DP-6", Path: "mst:62-1-9", EDID: info(t, cheap)},
			},
			want: []want{{"DP-5", fpCheap + "@mst:62-1-8", false}, {"DP-6", fpCheap + "@mst:62-1-9", false}},
		},
		{
			name: "vendor reusing one serial on every unit is a collision too",
			in: []monitor.Output{
				{Connector: "DP-1", EDID: info(t, dellA)},
				{Connector: "DP-2", EDID: info(t, dellA)},
			},
			want: []want{{"DP-1", fpA + "@DP-1", false}, {"DP-2", fpA + "@DP-2", false}},
		},
		{
			name: "no EDID falls back to connector",
			in:   []monitor.Output{{Connector: "HDMI-A-1"}},
			want: []want{{"HDMI-A-1", "connector:HDMI-A-1", false}},
		},
		{
			name: "two built-in panels are not both called builtin",
			in: []monitor.Output{
				{Connector: "eDP-1", EDID: info(t, dellA)},
				{Connector: "eDP-2", EDID: info(t, dellB)},
			},
			want: []want{{"eDP-1", fpA, true}, {"eDP-2", fpB, true}},
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := monitor.Identify(tt.in)
			if len(got) != len(tt.want) {
				t.Fatalf("got %d outputs, want %d", len(got), len(tt.want))
			}
			for i, w := range tt.want {
				g := got[i]
				if g.Connector != w.connector || g.ID != w.id || g.Strong != w.strong {
					t.Errorf("[%d] got {%s %s strong=%v}, want {%s %s strong=%v}",
						i, g.Connector, g.ID, g.Strong, w.connector, w.id, w.strong)
				}
			}
		})
	}
}

func TestIdentifyIsStableAcrossProbeOrder(t *testing.T) {
	a := []monitor.Output{{Connector: "DP-1", EDID: info(t, dellA)}, {Connector: "DP-2", EDID: info(t, dellB)}}
	b := []monitor.Output{a[1], a[0]}
	ga, gb := monitor.Identify(a), monitor.Identify(b)
	for i := range ga {
		if ga[i].ID != gb[i].ID {
			t.Fatalf("order changed IDs: %s vs %s", ga[i].ID, gb[i].ID)
		}
	}
}

func TestIdentifyFollowsMonitorAcrossPorts(t *testing.T) {
	before := monitor.Identify([]monitor.Output{{Connector: "DP-1", EDID: info(t, dellA)}})
	after := monitor.Identify([]monitor.Output{{Connector: "HDMI-A-1", EDID: info(t, dellA)}})
	if before[0].ID != after[0].ID {
		t.Errorf("moving a monitor to another port changed its ID: %s -> %s", before[0].ID, after[0].ID)
	}
	if strings.Contains(before[0].ID, "DP-1") {
		t.Errorf("strong ID must not contain the connector: %s", before[0].ID)
	}
}

func TestIdentifyDoesNotMutateInput(t *testing.T) {
	in := []monitor.Output{{Connector: "DP-2"}, {Connector: "DP-1"}}
	monitor.Identify(in)
	if in[0].Connector != "DP-2" {
		t.Error("Identify reordered the caller's slice")
	}
}
