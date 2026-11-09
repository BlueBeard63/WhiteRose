package session

import "testing"

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
