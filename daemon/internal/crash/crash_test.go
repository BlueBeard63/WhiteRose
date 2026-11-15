package crash

import (
	"os"
	"strings"
	"testing"
)

func TestWriteListPrune(t *testing.T) {
	state := t.TempDir()
	for i := range 7 {
		if _, err := Write(state, Record{What: Shell, Signal: 11, Time: int64(1794571766000 + i)}, []string{"panicked", "here"}); err != nil {
			t.Fatal(err)
		}
	}
	rs := List(state)
	if len(rs) != Keep || rs[0].Time != 1794571766006 || rs[Keep-1].Time != 1794571766002 {
		t.Fatalf("%+v", rs)
	}
	if b, _ := os.ReadFile(File(state, rs[0].ID, "output.txt")); string(b) != "panicked\nhere\n" {
		t.Errorf("output %q", b)
	}
	if _, err := Load(state, "../../etc"); err == nil {
		t.Error("loaded a path")
	}
	if _, err := Write(state, Record{What: "../x"}, nil); err == nil {
		t.Error("wrote a path")
	}
}

func TestDescribe(t *testing.T) {
	for r, want := range map[Record]string{
		{What: Shell, Signal: 11}:      "The shell crashed (signal 11, SIGSEGV)",
		{What: Compositor, Code: 101}:  "The compositor exited with code 101",
		{What: Daemon, Signal: 10}:     "The daemon (whiterosed) crashed (signal 10)",
		{What: Manual}:                 "Snapshot you took",
		{What: Memory, App: "firefox"}: "Closed firefox to free memory",
	} {
		if got := r.Describe(); got != want || strings.Contains(got, "—") {
			t.Errorf("%+v: %q, want %q", r, got, want)
		}
	}
}
