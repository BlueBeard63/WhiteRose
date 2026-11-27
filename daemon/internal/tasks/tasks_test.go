package tasks

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func put(t *testing.T, path, body string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(body), 0644); err != nil {
		t.Fatal(err)
	}
}
func fixture(t *testing.T) *Sampler {
	t.Helper()
	root := t.TempDir()
	s := New(filepath.Join(root, "slice"))
	s.Proc = filepath.Join(root, "proc")
	s.Sys = filepath.Join(root, "sys")
	put(t, filepath.Join(s.Proc, "stat"), "cpu 100 0 100 800 0 0 0 0\ncpu0 100 0 100 800 0 0 0 0\n")
	put(t, filepath.Join(s.Proc, "meminfo"), "MemTotal: 1024 kB\nMemAvailable: 512 kB\n")
	unit := filepath.Join(s.Slice, "app-whiterose-test-123-0.scope")
	put(t, filepath.Join(unit, "cgroup.procs"), "123\n")
	put(t, filepath.Join(unit, "memory.stat"), "anon 2048\nfile 1000000\n")
	put(t, filepath.Join(unit, "cpu.stat"), "usage_usec 10000\n")
	put(t, filepath.Join(unit, "io.stat"), "8:0 rbytes=1024 wbytes=2048\n")
	put(t, filepath.Join(s.Proc, "123/status"), fmt.Sprintf("Uid:\t%d %d %d %d\nRssAnon: 2 kB\n", s.UID, s.UID, s.UID, s.UID))
	f := strings.Fields("S 1 0 0 0 0 0 0 0 0 0 10 20 0 0 0 0 0 0 777 0 2")
	put(t, filepath.Join(s.Proc, "123/stat"), "123 (a tricky ) name) "+strings.Join(f, " "))
	return s
}
func TestSamplesAndCounterReset(t *testing.T) {
	s := fixture(t)
	first := s.Sample()
	if len(first.Apps) != 1 || *first.Apps[0].Memory != 2048 || first.Apps[0].CPU != nil || first.CPU[0].Value != nil {
		t.Fatal(first)
	}
	if first.Apps[0].Processes[0].Name != "a tricky ) name" || first.Apps[0].Processes[0].Start != 777 {
		t.Fatal(first.Apps)
	}
	if first.Memory.Value == nil || *first.Memory.Value != 50 {
		t.Fatal(first.Memory)
	}
	put(t, filepath.Join(s.Proc, "stat"), "cpu 120 0 120 860 0 0 0 0\ncpu0 120 0 120 860 0 0 0 0\n")
	s.last = time.Now().Add(-time.Second)
	second := s.Sample()
	if second.CPU[0].Value == nil || *second.CPU[0].Value != 40 {
		t.Fatal(second.CPU)
	}
	put(t, filepath.Join(s.Proc, "stat"), "cpu 1 0 1 8\n")
	s.last = time.Now().Add(-time.Second)
	third := s.Sample()
	if third.CPU[0].Value != nil {
		t.Fatal("reset displayed as activity", third.CPU)
	}
	if _, err := json.Marshal(third); err != nil {
		t.Fatal(err)
	}
}
func TestFallbackAndSafety(t *testing.T) {
	s := fixture(t)
	a := s.Sample().Apps[0]
	for _, unit := range []string{"../" + a.Unit, "pid:123", "app-whiterose-test-123-0.scope/.."} {
		if s.End(unit, a.Token, false) == nil {
			t.Fatal("unsafe target accepted", unit)
		}
	}
	if s.End(a.Unit, "stale", true) == nil {
		t.Fatal("stale scope accepted")
	}
	put(t, filepath.Join(s.Proc, "123/stat"), "123 (whiterose-wm) S 1 0 0 0 0 0 0 0 0 0 10 20 0 0 0 0 0 0 777 0 2")
	s.last = time.Time{}
	a = s.Sample().Apps[0]
	if a.CanEnd {
		t.Fatal("session component is controllable")
	}
	s.Slice = filepath.Join(t.TempDir(), "missing")
	s.last = time.Time{}
	state := s.Sample()
	if state.Scoped || len(state.Apps) != 1 || state.Apps[0].CanEnd {
		t.Fatal(state)
	}
}
func TestHistoryAndRates(t *testing.T) {
	var h []*float64
	for i := 0; i < 90; i++ {
		h = history(h, ptr(float64(i)))
	}
	if len(h) != 60 || *h[0] != 30 {
		t.Fatal(h)
	}
	if rate(2, 3, 1, true) != nil || rate(2, 1, 0, true) != nil || rate(2, 1, 1, false) != nil {
		t.Fatal("invalid rate")
	}
	if *rate(4096, 2048, 2, true) != 1024 {
		t.Fatal("rate")
	}
}
