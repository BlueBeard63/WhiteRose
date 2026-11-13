// Package crash keeps a record of each time part of the session died: the
// compositor (written by whiterose-launch), the shell or the daemon
// (written by the compositor's supervisor, compositor/src/shell_supervisor.rs),
// a memory-pressure close, or a snapshot you asked for (SUPER+SHIFT+B).
//
// Each record is a folder, ~/.local/state/whiterose/crashes/<ms>-<what>/,
// with crash.json (this package's Record; the compositor writes the same
// fields), output.txt (what it printed last, when the writer had it) and,
// once whiterosed has taken it, snapshot.json. The newest Keep are kept.
// Nothing here is sent anywhere: a bug report is what sends them.
package crash

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
	"time"
)

// Keep is how many records are kept.
const Keep = 5

// What died (or why the record was made).
const (
	Compositor = "compositor"
	Shell      = "shell"
	Daemon     = "daemon"
	Manual     = "manual" // a snapshot you asked for
	Memory     = "memory" // an app closed under memory pressure
)

type Record struct {
	What    string `json:"what"`
	Code    int    `json:"code,omitempty"`   // exit code (when it exited)
	Signal  int    `json:"signal,omitempty"` // or the signal that ended it
	Time    int64  `json:"time"`             // unix ms
	Version string `json:"version,omitempty"`
	// What it was about, for memory closes ("org.mozilla.firefox").
	App string `json:"app,omitempty"`

	ID string `json:"id"` // its folder's name (filled in by List)
}

// Dir is where the records are.
func Dir(stateHome string) string { return filepath.Join(stateHome, "whiterose", "crashes") }

var idRe = regexp.MustCompile(`^[0-9]{13}-[a-z]{1,20}$`)

// Write records r (with `output`, may be nil) and drops the oldest past Keep.
func Write(stateHome string, r Record, output []string) (string, error) {
	if r.Time == 0 {
		r.Time = time.Now().UnixMilli()
	}
	id := fmt.Sprintf("%013d-%s", r.Time, r.What)
	if !idRe.MatchString(id) {
		return "", fmt.Errorf("bad record %q", id)
	}
	dir := filepath.Join(Dir(stateHome), id)
	if err := os.MkdirAll(dir, 0o700); err != nil {
		return "", err
	}
	r.ID = ""
	b, _ := json.MarshalIndent(r, "", "  ")
	if err := os.WriteFile(filepath.Join(dir, "crash.json"), b, 0o600); err != nil {
		return "", err
	}
	if len(output) > 0 {
		_ = os.WriteFile(filepath.Join(dir, "output.txt"), []byte(strings.Join(output, "\n")+"\n"), 0o600)
	}
	Prune(stateHome)
	return id, nil
}

// List is every record, newest first.
func List(stateHome string) []Record {
	entries, _ := os.ReadDir(Dir(stateHome))
	var out []Record
	for _, e := range entries {
		if !e.IsDir() || !idRe.MatchString(e.Name()) {
			continue
		}
		r, err := Load(stateHome, e.Name())
		if err != nil {
			continue
		}
		out = append(out, r)
	}
	sort.Slice(out, func(i, j int) bool { return out[i].ID > out[j].ID })
	return out
}

// Load reads one record.
func Load(stateHome, id string) (Record, error) {
	if !idRe.MatchString(id) {
		return Record{}, errors.New("no such record")
	}
	b, err := os.ReadFile(filepath.Join(Dir(stateHome), id, "crash.json"))
	if err != nil {
		return Record{}, err
	}
	var r Record
	if err := json.Unmarshal(b, &r); err != nil {
		return Record{}, err
	}
	r.ID = id
	return r, nil
}

// File is one of a record's files ("output.txt", "snapshot.json").
func File(stateHome, id, name string) string {
	return filepath.Join(Dir(stateHome), id, name)
}

// Prune drops all but the newest Keep (and anything that isn't a record).
func Prune(stateHome string) {
	entries, _ := os.ReadDir(Dir(stateHome))
	var ids []string
	for _, e := range entries {
		if e.IsDir() && idRe.MatchString(e.Name()) {
			ids = append(ids, e.Name())
		}
	}
	sort.Sort(sort.Reverse(sort.StringSlice(ids)))
	for _, id := range ids[min(len(ids), Keep):] {
		os.RemoveAll(filepath.Join(Dir(stateHome), id))
	}
}

// Describe: "Shell crashed (signal 11, SIGSEGV)", "Compositor exited with code 101".
func (r Record) Describe() string {
	name := map[string]string{Compositor: "The compositor", Shell: "The shell", Daemon: "The daemon (whiterosed)"}[r.What]
	switch r.What {
	case Manual:
		return "Snapshot you took"
	case Memory:
		return "Closed " + r.App + " to free memory"
	}
	if name == "" {
		name = r.What
	}
	if r.Signal != 0 {
		if s := signalName(r.Signal); s != "" {
			return fmt.Sprintf("%s crashed (signal %d, %s)", name, r.Signal, s)
		}
		return fmt.Sprintf("%s crashed (signal %d)", name, r.Signal)
	}
	return fmt.Sprintf("%s exited with code %d", name, r.Code)
}

func signalName(n int) string {
	names := map[int]string{1: "SIGHUP", 2: "SIGINT", 3: "SIGQUIT", 4: "SIGILL", 5: "SIGTRAP", 6: "SIGABRT", 7: "SIGBUS",
		8: "SIGFPE", 9: "SIGKILL", 11: "SIGSEGV", 13: "SIGPIPE", 15: "SIGTERM"}
	if s, ok := names[n]; ok {
		return s
	}
	return ""
}
