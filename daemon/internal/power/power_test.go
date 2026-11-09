package power

import "testing"

func TestBarsResampleHistory(t *testing.T) {
	now := uint32(100000)
	// UPower gives newest first, at irregular times.
	raw := []Point{
		{Time: now - 100, Percent: 82, Charging: false},
		{Time: now - 3*3600, Percent: 60, Charging: true},
		{Time: now - 20*3600, Percent: 30, Charging: false},
	}
	b := Bars(raw, now, 24*3600, 24)
	if len(b) != 24 {
		t.Fatalf("got %d bars", len(b))
	}
	// Bar i ends at now-24h+(i+1)h, so the sample 20 h ago first shows in bar 3.
	if b[0].Percent != -1 || b[2].Percent != -1 {
		t.Errorf("before the first sample there's no data: %v %v", b[0], b[2])
	}
	if b[3].Percent != 30 {
		t.Errorf("bar 3 ends exactly at the 20 h old sample: %+v", b[3])
	}
	if b[4].Percent != 30 || b[4].Charging {
		t.Errorf("hour 4 (20 h ago): %+v", b[4])
	}
	if b[21].Percent != 60 || !b[21].Charging {
		t.Errorf("3 h ago: %+v", b[21])
	}
	if b[23].Percent != 82 || b[23].Time != now {
		t.Errorf("now: %+v", b[23])
	}
	for i := 1; i < len(b); i++ {
		if b[i].Time <= b[i-1].Time {
			t.Fatal("bars must be oldest first")
		}
	}
	if len(Bars(nil, now, 3600, 6)) != 6 {
		t.Fatal("no history still gives empty bars")
	}
}
