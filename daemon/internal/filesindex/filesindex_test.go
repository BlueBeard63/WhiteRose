package filesindex

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
	"testing/fstest"
	"time"
)

func supplies(kv map[string]string) fstest.MapFS {
	m := fstest.MapFS{}
	for k, v := range kv {
		m[k] = &fstest.MapFile{Data: []byte(v + "\n")}
	}
	return m
}

func TestOnBattery(t *testing.T) {
	for _, c := range []struct {
		name string
		fs   fstest.MapFS
		want bool
	}{
		{"laptop on mains", supplies(map[string]string{"ADP1/type": "Mains", "ADP1/online": "1", "BAT1/type": "Battery"}), false},
		{"laptop unplugged", supplies(map[string]string{"ADP1/type": "Mains", "ADP1/online": "0", "BAT1/type": "Battery"}), true},
		{"USB-C charger", supplies(map[string]string{"ADP1/type": "Mains", "ADP1/online": "0", "ucsi/type": "USB", "ucsi/online": "1", "BAT1/type": "Battery"}), false},
		{"desktop", supplies(map[string]string{}), false},
		{"desktop with a wireless mouse", supplies(map[string]string{"hid/type": "Battery", "hid/scope": "Device"}), false},
		{"laptop with no adapter listed", supplies(map[string]string{"BAT0/type": "Battery"}), true},
	} {
		if got := OnBattery(c.fs); got != c.want {
			t.Errorf("%s: OnBattery = %v", c.name, got)
		}
	}
}

func TestDueAndRead(t *testing.T) {
	ix := filepath.Join(t.TempDir(), "index")
	now := time.Now()
	if !Due(ix, now) {
		t.Error("no index isn't due")
	}
	if s := Read(ix); s.Made != 0 || s.Progress != nil {
		t.Errorf("no index: %+v", s)
	}
	os.WriteFile(ix, nil, 0o600)
	os.Chtimes(ix, now.Add(-time.Minute), now.Add(-time.Minute))
	if Due(ix, now) {
		t.Error("a minute-old index is due")
	}
	os.Chtimes(ix, now.Add(-2*time.Hour), now.Add(-2*time.Hour))
	if !Due(ix, now) {
		t.Error("a two-hour-old index isn't due")
	}

	// Being made: by a live process only.
	p := Progress{PID: os.Getpid(), Started: 1, Count: 420, Expected: 1000}
	b, _ := json.Marshal(p)
	os.WriteFile(ix+".progress", b, 0o600)
	if s := Read(ix); s.Progress == nil || *s.Progress != p || s.Made == 0 {
		t.Errorf("being made: %+v", s)
	}
	p.PID = 1 << 30
	b, _ = json.Marshal(p)
	os.WriteFile(ix+".progress", b, 0o600)
	if s := Read(ix); s.Progress != nil {
		t.Errorf("a dead indexer counts: %+v", s.Progress)
	}
}

// A stand-in whiterose-files: --index leaves a mark.
func fakeFiles(t *testing.T) (cmd, mark string) {
	dir := t.TempDir()
	cmd, mark = filepath.Join(dir, "whiterose-files"), filepath.Join(dir, "ran")
	os.WriteFile(cmd, []byte("#!/bin/sh\n[ \"$1\" = --index ] && echo x >> "+mark+"\n"), 0o755)
	return cmd, mark
}

func waitFor(t *testing.T, mark string, want bool) {
	t.Helper()
	for range 50 {
		_, err := os.Stat(mark)
		if (err == nil) == want {
			return
		}
		time.Sleep(20 * time.Millisecond)
	}
	t.Fatalf("ran: want %v", want)
}

func TestTickRunsOnlyWhenDueAndPluggedIn(t *testing.T) {
	files, mark := fakeFiles(t)
	k := &Keeper{
		Files: files,
		Index: filepath.Join(t.TempDir(), "index"),
		Power: supplies(map[string]string{"ADP1/type": "Mains", "ADP1/online": "0", "BAT1/type": "Battery"}),
		Log:   t.Logf,
	}
	k.Tick(time.Now())
	time.Sleep(100 * time.Millisecond)
	waitFor(t, mark, false) // on battery: waits

	k.Power = supplies(map[string]string{"ADP1/type": "Mains", "ADP1/online": "1", "BAT1/type": "Battery"})
	k.Tick(time.Now())
	waitFor(t, mark, true)

	// Fresh: nothing to do.
	os.Remove(mark)
	os.WriteFile(k.Index, nil, 0o600)
	k.Tick(time.Now())
	time.Sleep(100 * time.Millisecond)
	waitFor(t, mark, false)

	// Asked for: on battery too.
	k.Power = supplies(map[string]string{"BAT1/type": "Battery"})
	for k.busy() {
		time.Sleep(10 * time.Millisecond)
	}
	k.Now()
	waitFor(t, mark, true)
}

func (k *Keeper) busy() bool {
	k.mu.Lock()
	defer k.mu.Unlock()
	return k.running
}
