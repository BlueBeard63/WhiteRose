package update

import (
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

func TestKeepTwo(t *testing.T) {
	cases := []struct {
		installed []string
		running   string
		keep      []string
		remove    []string
	}{
		{[]string{"0.4.2"}, "0.4.2", []string{"0.4.2"}, nil},
		{[]string{"0.4.2", "0.5.0"}, "0.4.2", []string{"0.5.0", "0.4.2"}, nil},
		{[]string{"0.3.0", "0.4.2", "0.5.0"}, "0.4.2", []string{"0.5.0", "0.4.2"}, []string{"0.3.0"}},
		{[]string{"0.3.0", "0.4.2", "0.5.0"}, "0.5.0", []string{"0.5.0", "0.4.2"}, []string{"0.3.0"}},
		// Running an older one (rolled back): it stays, the rest go.
		{[]string{"0.3.0", "0.4.2", "0.5.0"}, "0.3.0", []string{"0.5.0", "0.3.0"}, []string{"0.4.2"}},
		{[]string{"0.10.0", "0.9.0", "0.8.1", "0.2.0"}, "", []string{"0.10.0", "0.9.0"}, []string{"0.2.0", "0.8.1"}},
		{nil, "", nil, nil},
	}
	for _, c := range cases {
		if got := Keep(c.installed, c.running); !reflect.DeepEqual(got, c.keep) {
			t.Errorf("Keep(%v, %q) = %v, want %v", c.installed, c.running, got, c.keep)
		}
		if got := Remove(c.installed, c.running); !reflect.DeepEqual(got, c.remove) {
			t.Errorf("Remove(%v, %q) = %v, want %v", c.installed, c.running, got, c.remove)
		}
	}
}

func TestInstalled(t *testing.T) {
	root := t.TempDir()
	for _, v := range []string{"0.10.0", "0.9.1", "0.5.0", "half", "../x"} {
		os.MkdirAll(filepath.Join(root, v, "bin"), 0o755)
	}
	for _, v := range []string{"0.10.0", "0.9.1"} {
		os.WriteFile(filepath.Join(root, v, "bin", "whiterose-wm"), nil, 0o755)
	}
	// 0.5.0 has no compositor: half-installed, not counted.
	if got := Installed(root); !reflect.DeepEqual(got, []string{"0.9.1", "0.10.0"}) {
		t.Errorf("installed %v", got)
	}
	if Installed(filepath.Join(root, "missing")) != nil {
		t.Error("missing root")
	}
}
