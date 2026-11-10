package audio

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

// testdata/laptop.json: an Intel laptop card (SOF, UCM) with a screen on
// HDMI 1, nothing in the headphone jack, in the setup with Headphones (the
// Speaker is in the other setup).
func laptop(t *testing.T) []byte {
	b, err := os.ReadFile("testdata/laptop.json")
	if err != nil {
		t.Fatal(err)
	}
	return b
}

func byKey(ps []Port) map[string]Port {
	m := map[string]Port{}
	for _, p := range ps {
		m[p.Key[strings.Index(p.Key, "#")+1:]] = p
	}
	return m
}

func TestParseListsEveryPlaceIncludingOtherSetups(t *testing.T) {
	ps, err := Parse(laptop(t), []string{"T34w-30"})
	if err != nil {
		t.Fatal(err)
	}
	m := byKey(ps)
	sp := m["[Out] Speaker"]
	if sp.Node != "" || sp.Profile != 2 || sp.Device != 61 || sp.Available != "unknown" {
		t.Errorf("speaker: %+v (want no node yet, setup 2 of device 61)", sp)
	}
	if hp := m["[Out] Headphones"]; hp.Available != "no" || hp.Node == "" {
		t.Errorf("headphones: %+v (want unplugged, with its node)", hp)
	}
	if h := m["[Out] HDMI1"]; h.Label != "T34w-30" || h.Available != "yes" || !strings.HasSuffix(h.Node, "HDMI1__sink") {
		t.Errorf("HDMI 1: %+v (want named after its screen)", h)
	}
	if h := m["[Out] HDMI2"]; h.Label != "HDMI / DisplayPort 2 Output" || h.Available != "no" {
		t.Errorf("HDMI 2: %+v", h)
	}
	if mic := m["[In] Mic2"]; mic.Available != "no" || mic.Direction != "in" {
		t.Errorf("headset mic: %+v (want unplugged input)", mic)
	}
	if mic := m["[In] Mic1"]; mic.Label != "Digital Microphone" || mic.Node == "" {
		t.Errorf("built-in mic: %+v", mic)
	}
	// Outputs first.
	seenIn := false
	for _, p := range ps {
		if p.Direction == "in" {
			seenIn = true
		} else if seenIn {
			t.Errorf("output %s after an input", p.Label)
		}
	}
}

func TestScreenNamesOnlyWhenClear(t *testing.T) {
	ps, _ := Parse(laptop(t), []string{"A", "B"})
	if byKey(ps)["[Out] HDMI1"].Label != "HDMI / DisplayPort 1 Output" {
		t.Error("two screens with sound: can't tell which port is which")
	}
}

func TestScreensReadsELD(t *testing.T) {
	d := t.TempDir()
	os.MkdirAll(filepath.Join(d, "card0"), 0o755)
	os.WriteFile(filepath.Join(d, "card0", "eld#2.4"), []byte("monitor_present\t\t1\neld_valid\t\t1\nmonitor_name\t\tT34w-30\n"), 0o644)
	os.WriteFile(filepath.Join(d, "card0", "eld#2.5"), []byte("monitor_present\t\t0\neld_valid\t\t0\n"), 0o644)
	if s := Screens(d); len(s) != 1 || s[0] != "T34w-30" {
		t.Errorf("screens: %v", s)
	}
}

// A fake PipeWire: switching the setup brings the Speaker's node.
type fake struct {
	t     *testing.T
	dump  []byte
	calls []string
}

func (f *fake) run(name string, args ...string) ([]byte, error) {
	f.calls = append(f.calls, name+" "+strings.Join(args, " "))
	if name == "wpctl" && len(args) == 3 && args[0] == "set-profile" {
		var objs []map[string]any
		json.Unmarshal(f.dump, &objs)
		// The Headphones setup goes; the Speaker's node arrives.
		var keep []map[string]any
		for _, o := range objs {
			if p, ok := o["info"].(map[string]any)["props"].(map[string]any); ok && strings.Contains(fmt.Sprint(p["node.name"]), "Headphones") {
				continue
			}
			keep = append(keep, o)
		}
		for _, o := range keep {
			if o["type"] == "PipeWire:Interface:Device" {
				o["info"].(map[string]any)["params"].(map[string]any)["Profile"] = []any{map[string]any{"index": 2}}
			}
		}
		keep = append(keep, map[string]any{"id": 99, "type": "PipeWire:Interface:Node", "info": map[string]any{"props": map[string]any{
			"media.class": "Audio/Sink", "node.name": "alsa_output.x.HiFi__Speaker__sink", "device.id": 61, "card.profile.device": 3,
		}}})
		f.dump, _ = json.Marshal(keep)
	}
	if name == "pw-dump" {
		return f.dump, nil
	}
	return nil, nil
}

func TestUseSwitchesTheSetupThenTheDefault(t *testing.T) {
	f := &fake{t: t, dump: laptop(t)}
	ps, _ := Parse(f.dump, nil)
	if err := Use(f.run, byKey(ps)["[Out] Speaker"].Key, time.Second); err != nil {
		t.Fatal(err, f.calls)
	}
	want := []string{"wpctl set-profile 61 2", "wpctl set-default 99"}
	got := []string{}
	for _, c := range f.calls {
		if strings.HasPrefix(c, "wpctl") {
			got = append(got, c)
		}
	}
	if strings.Join(got, ";") != strings.Join(want, ";") {
		t.Errorf("calls %v, want %v", got, want)
	}
}

func TestUseRefusesUnpluggedAndPicksExistingNodes(t *testing.T) {
	f := &fake{t: t, dump: laptop(t)}
	ps, _ := Parse(f.dump, nil)
	m := byKey(ps)
	if err := Use(f.run, m["[In] Mic2"].Key, time.Second); err != ErrUnplugged {
		t.Errorf("unplugged headset mic: %v", err)
	}
	if err := Use(f.run, m["[In] Mic1"].Key, time.Second); err != nil {
		t.Fatal(err)
	}
	if last := f.calls[len(f.calls)-1]; last != "wpctl set-default 34" {
		t.Errorf("last call %q", last)
	}
}
