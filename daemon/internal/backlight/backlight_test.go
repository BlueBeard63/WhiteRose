package backlight

import (
	"testing"
	"testing/fstest"
)

func dev(t, cur, maxv string) map[string]*fstest.MapFile {
	return map[string]*fstest.MapFile{
		"type": {Data: []byte(t + "\n")}, "brightness": {Data: []byte(cur + "\n")}, "max_brightness": {Data: []byte(maxv + "\n")},
	}
}

func tree(devs map[string]map[string]*fstest.MapFile) fstest.MapFS {
	m := fstest.MapFS{}
	for name, files := range devs {
		for f, data := range files {
			m[name+"/"+f] = data
		}
	}
	return m
}

func TestFindPrefersFirmwareThenPlatformThenRaw(t *testing.T) {
	sys := tree(map[string]map[string]*fstest.MapFile{
		"intel_backlight": dev("raw", "236", "400"),
		"acpi_video0":     dev("firmware", "5", "10"),
		"dell_backlight":  dev("platform", "3", "15"),
	})
	d, err := Find(sys)
	if err != nil || d.Name != "acpi_video0" {
		t.Fatalf("got %+v, %v; want acpi_video0", d, err)
	}
	delete(sys, "acpi_video0/type")
	delete(sys, "acpi_video0/brightness")
	delete(sys, "acpi_video0/max_brightness")
	if d, _ := Find(sys); d.Name != "dell_backlight" {
		t.Fatalf("got %s, want dell_backlight", d.Name)
	}
}

func TestFindSkipsBrokenAndReportsNone(t *testing.T) {
	sys := tree(map[string]map[string]*fstest.MapFile{"broken": dev("raw", "1", "0")})
	if _, err := Find(sys); err != ErrNone {
		t.Fatalf("err = %v, want ErrNone", err)
	}
	if _, err := Find(fstest.MapFS{}); err != ErrNone {
		t.Fatalf("empty: err = %v, want ErrNone", err)
	}
}

func TestPercentAndRaw(t *testing.T) {
	d := Device{Current: 236, Max: 400}
	if p := d.Percent(); p != 59 {
		t.Fatalf("percent = %d, want 59", p)
	}
	for _, c := range []struct{ pct, raw int }{{0, 0}, {1, 4}, {50, 200}, {100, 400}, {150, 400}, {-5, 0}} {
		if r := d.Raw(c.pct); r != c.raw {
			t.Errorf("Raw(%d) = %d, want %d", c.pct, r, c.raw)
		}
	}
	tiny := Device{Max: 7}
	if r := tiny.Raw(1); r != 1 {
		t.Errorf("1%% of 7 = %d, want 1 (never fully off)", r)
	}
}
