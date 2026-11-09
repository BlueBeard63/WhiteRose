package rfkill

import (
	"encoding/binary"
	"io"
	"testing"
	"testing/fstest"
)

func radio(name, typ, soft, hard string) fstest.MapFS {
	return fstest.MapFS{
		name + "/name": {Data: []byte(name + "\n")}, name + "/type": {Data: []byte(typ + "\n")},
		name + "/soft": {Data: []byte(soft + "\n")}, name + "/hard": {Data: []byte(hard + "\n")},
	}
}

func merge(fss ...fstest.MapFS) fstest.MapFS {
	m := fstest.MapFS{}
	for _, f := range fss {
		for k, v := range f {
			m[k] = v
		}
	}
	return m
}

func TestAirplaneNeedsEveryRadioBlocked(t *testing.T) {
	cases := []struct {
		name string
		sys  fstest.MapFS
		want bool
	}{
		{"no radios", fstest.MapFS{}, false},
		{"all soft-blocked", merge(radio("phy0", "wlan", "1", "0"), radio("hci0", "bluetooth", "1", "0")), true},
		{"bluetooth still on", merge(radio("phy0", "wlan", "1", "0"), radio("hci0", "bluetooth", "0", "0")), false},
		{"hardware switch counts as off", merge(radio("phy0", "wlan", "0", "1"), radio("hci0", "bluetooth", "1", "0")), true},
	}
	for _, c := range cases {
		radios, err := Scan(c.sys)
		if err != nil {
			t.Fatal(err)
		}
		if got := Airplane(radios); got != c.want {
			t.Errorf("%s: Airplane = %v, want %v", c.name, got, c.want)
		}
	}
}

func TestSetAllEventMatchesKernelABI(t *testing.T) {
	ev := SetAllEvent(true)
	if len(ev) != 8 || ev[4] != 0 || ev[5] != 3 || ev[6] != 1 || ev[7] != 0 {
		t.Fatalf("block-all event = %v", ev)
	}
	if SetAllEvent(false)[6] != 0 {
		t.Fatal("unblock event must clear soft")
	}
}

// Kernels since 5.11 send 9-byte events (with hard_block_reasons).
type chunks [][]byte

func (c *chunks) Read(p []byte) (int, error) {
	if len(*c) == 0 {
		return 0, io.EOF
	}
	n := copy(p, (*c)[0])
	*c = (*c)[1:]
	return n, nil
}

func TestReadEventsHandlesOldAndNewEventSizes(t *testing.T) {
	ev := func(idx uint32, soft byte, extra int) []byte {
		b := make([]byte, 8+extra)
		binary.NativeEndian.PutUint32(b, idx)
		b[4], b[5], b[6] = 1, 2, soft
		return b
	}
	src := chunks{ev(1, 1, 0), ev(2, 0, 1), []byte{1, 2}}
	var got []Event
	err := ReadEvents(&src, func(e Event) { got = append(got, e) })
	if err != io.EOF || len(got) != 2 || got[0].Idx != 1 || got[0].Soft != 1 || got[1].Idx != 2 || got[1].Soft != 0 {
		t.Fatalf("got %+v, err %v", got, err)
	}
}
