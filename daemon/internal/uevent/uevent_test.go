package uevent

import "testing"

func TestParseKernelEvent(t *testing.T) {
	msg := []byte("change@/devices/pci0000:00/0000:00:02.0/drm/card1/card1-eDP-1/intel_backlight\x00ACTION=change\x00" +
		"DEVPATH=/devices/pci0000:00/0000:00:02.0/drm/card1/card1-eDP-1/intel_backlight\x00SUBSYSTEM=backlight\x00SOURCE=sysfs\x00SEQNUM=4242\x00")
	ev := Parse(msg)
	if ev == nil || ev.Action != "change" || ev.Props["SUBSYSTEM"] != "backlight" || ev.Props["SOURCE"] != "sysfs" {
		t.Fatalf("got %+v", ev)
	}
}

func TestParseIgnoresUdevRelays(t *testing.T) {
	if ev := Parse([]byte("libudev\x00\xfe\xed\xca\xfe")); ev != nil {
		t.Fatalf("udev relay parsed as %+v", ev)
	}
	if ev := Parse(nil); ev != nil {
		t.Fatal("empty message parsed")
	}
}

func FuzzParse(f *testing.F) {
	f.Add([]byte("add@/x\x00ACTION=add\x00A=B"))
	f.Add([]byte("@\x00="))
	f.Fuzz(func(t *testing.T, b []byte) { Parse(b) })
}
