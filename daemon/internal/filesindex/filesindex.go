// Package filesindex keeps Files' search index fresh without Files open:
// whiterosed runs `whiterose-files --index` (its own process, at the
// lowest CPU and disk priority, one at a time) when the index is over an
// hour old, but only on mains power: on battery it waits until you plug in.
// Asked for (Settings › Files › Search › Make again, or what's indexed
// changed), it runs straight away, on battery too.
//
// The index and its progress file are Files' (apps/files/src/indexer.rs);
// this only reads them.
package filesindex

import (
	"encoding/json"
	"io/fs"
	"os"
	"os/exec"
	"path"
	"path/filepath"
	"strings"
	"sync"
	"syscall"
	"time"
)

// Stale is how old the index gets before it's made again.
const Stale = time.Hour

// File is the index: $XDG_CACHE_HOME/whiterose/files/index.
func File() string {
	dir := os.Getenv("XDG_CACHE_HOME")
	if !filepath.IsAbs(dir) {
		home, _ := os.UserHomeDir()
		dir = filepath.Join(home, ".cache")
	}
	return filepath.Join(dir, "whiterose", "files", "index")
}

// Progress is what the indexer writes while it runs (index.progress).
type Progress struct {
	PID      int     `json:"pid"`
	Started  float64 `json:"started"` // unix ms
	Count    uint64  `json:"count"`
	Expected uint64  `json:"expected"` // the index before's size; 0 the first time
}

// Status is the index for Settings: when it was made (unix s, 0: never)
// and, while one is being made, how far it's got.
type Status struct {
	Made     int64     `json:"made"`
	Progress *Progress `json:"progress"`
}

// Read is the index's status now.
func Read(index string) Status {
	var s Status
	if fi, err := os.Stat(index); err == nil {
		s.Made = fi.ModTime().Unix()
	}
	s.Progress = running(index)
	return s
}

// running: the progress file, while the process that wrote it is alive.
func running(index string) *Progress {
	b, err := os.ReadFile(strings.TrimSuffix(index, filepath.Ext(index)) + ".progress")
	if err != nil {
		return nil
	}
	var p Progress
	if json.Unmarshal(b, &p) != nil || p.PID <= 0 {
		return nil
	}
	if syscall.Kill(p.PID, 0) != nil {
		return nil
	}
	return &p
}

// Due: missing, or older than Stale at `now`.
func Due(index string, now time.Time) bool {
	fi, err := os.Stat(index)
	return err != nil || now.Sub(fi.ModTime()) > Stale
}

// OnBattery: running on the battery, from /sys/class/power_supply (as
// `sysfs`, rooted there). Anything that isn't a battery and is online
// (mains, a USB-C charger) means plugged in; no battery at all (a desktop)
// means plugged in too.
func OnBattery(sysfs fs.FS) bool {
	ents, err := fs.ReadDir(sysfs, ".")
	if err != nil {
		return false
	}
	battery := false
	for _, e := range ents {
		kind := read(sysfs, path.Join(e.Name(), "type"))
		switch {
		case kind == "Battery":
			if read(sysfs, path.Join(e.Name(), "scope")) != "Device" { // not a mouse's
				battery = true
			}
		case read(sysfs, path.Join(e.Name(), "online")) == "1":
			return false
		}
	}
	return battery
}

func read(sysfs fs.FS, name string) string {
	b, err := fs.ReadFile(sysfs, name)
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(b))
}

// Keeper runs the indexer.
type Keeper struct {
	// Files is the whiterose-files command.
	Files string
	// Index is the index file (File()).
	Index string
	// Power is /sys/class/power_supply.
	Power fs.FS
	// Log reports what went wrong.
	Log func(format string, args ...any)

	mu      sync.Mutex
	running bool
}

// Now makes the index (unless one is being made): it returns once it's
// started; the indexer carries on by itself.
func (k *Keeper) Now() {
	k.mu.Lock()
	if k.running {
		k.mu.Unlock()
		return
	}
	k.running = true
	k.mu.Unlock()
	cmd := exec.Command(k.Files, "--index")
	cmd.SysProcAttr = &syscall.SysProcAttr{Setsid: true}
	if err := cmd.Start(); err != nil {
		k.Log("search index: %v", err)
		k.mu.Lock()
		k.running = false
		k.mu.Unlock()
		return
	}
	go func() {
		if err := cmd.Wait(); err != nil {
			k.Log("search index: %v", err)
		}
		k.mu.Lock()
		k.running = false
		k.mu.Unlock()
	}()
}

// Tick makes the index if it's due and the computer is plugged in.
func (k *Keeper) Tick(now time.Time) {
	if Due(k.Index, now) && !OnBattery(k.Power) {
		k.Now()
	}
}

// Keep ticks every `every`, the first after `first`.
func (k *Keeper) Keep(first, every time.Duration) {
	time.Sleep(first)
	for {
		k.Tick(time.Now())
		time.Sleep(every)
	}
}
