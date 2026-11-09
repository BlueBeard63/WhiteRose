package session

import (
	"testing"

	"github.com/godbus/dbus/v5"
)

func TestFromSignal(t *testing.T) {
	for name, want := range map[string]Request{
		"org.freedesktop.login1.Session.Lock":   Lock,
		"org.freedesktop.login1.Session.Unlock": Unlock,
	} {
		if got, ok := FromSignal(name); !ok || got != want {
			t.Errorf("%s: got %q %v", name, got, ok)
		}
	}
	if _, ok := FromSignal("org.freedesktop.login1.Session.PauseDevice"); ok {
		t.Error("other signals are not requests")
	}
}

func TestSleepSignal(t *testing.T) {
	sig := func(body ...any) *dbus.Signal {
		return &dbus.Signal{Name: "org.freedesktop.login1.Manager.PrepareForSleep", Body: body}
	}
	if going, ok := sleepSignal(sig(true)); !ok || !going {
		t.Error("PrepareForSleep(true) is going to sleep")
	}
	if going, ok := sleepSignal(sig(false)); !ok || going {
		t.Error("PrepareForSleep(false) is waking up")
	}
	if _, ok := sleepSignal(sig("x")); ok {
		t.Error("a malformed signal was accepted")
	}
	if _, ok := sleepSignal(&dbus.Signal{Name: "org.freedesktop.login1.Session.Lock"}); ok {
		t.Error("another signal was taken for sleep")
	}
}
