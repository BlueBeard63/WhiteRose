package server

import "testing"

func TestSubsystemSnapshotsReplayAndStayOrdered(t *testing.T) {
	s, path, _ := setup(t)
	s.SetTopic("clocks", map[string]any{"revision": 1})
	a := dial(t, path)
	a.next() // settings
	first := a.next()
	if first["event"] != "clocks" || first["state"].(map[string]any)["revision"] != float64(1) {
		t.Fatal(first)
	}
	s.SetTopic("clocks", map[string]any{"revision": 2})
	second := a.next()
	if second["state"].(map[string]any)["revision"] != float64(2) {
		t.Fatal(second)
	}
	b := dial(t, path)
	b.next()
	if got := b.next()["state"].(map[string]any)["revision"]; got != float64(2) {
		t.Fatal(got)
	}
}
