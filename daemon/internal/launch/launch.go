// Package launch starts a WhiteRose session on the right version, and
// switches to a newly installed one safely (whiterose-launch):
//
//   - A new version is tried at most twice. Before a try, your settings are
//     snapshotted.
//   - A try passes when the compositor, daemon and shell each report ready
//     (files in WHITEROSE_READY_DIR) within 20 s and the session is still
//     running at 60 s. Then it's the version from now on.
//   - A try that fails is stopped, the snapshot is put back, and the version
//     before starts straight away, in the same login. The failure is kept for
//     the shell's "couldn't start" prompt; after two, it's left alone until
//     you choose (Try again / Stay).
//
// State is per user, in ~/.local/state/whiterose/version (key=value lines);
// nothing here needs root.
package launch

import (
	"bufio"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"slices"
	"sort"
	"strconv"
	"strings"
)

// Tries is how many times a new version is tried before it's left alone.
const Tries = 2

type State struct {
	Current  string // the version that works
	Pending  string // a new one to try
	Attempts int    // tries of Pending so far
	Failed   string // the version that last failed to start (for the prompt)
	Reason   string // why
	Skip     string // a version not to try (you stayed, or rolled back from it)
	Snapshot int64  // when settings were last snapshotted (unix s)
	// Automatic rollback off: a new version is switched to without the
	// health check (and nothing goes back by itself).
	NoRollback bool
}

func StateFile(stateHome string) string {
	return filepath.Join(stateHome, "whiterose", "version")
}

func Load(path string) State {
	var s State
	f, err := os.Open(path)
	if err != nil {
		return s
	}
	defer f.Close()
	sc := bufio.NewScanner(f)
	for sc.Scan() {
		k, v, ok := strings.Cut(sc.Text(), "=")
		if !ok {
			continue
		}
		switch k {
		case "current":
			s.Current = v
		case "pending":
			s.Pending = v
		case "attempts":
			s.Attempts, _ = strconv.Atoi(v)
		case "failed":
			s.Failed = v
		case "reason":
			s.Reason = v
		case "skip":
			s.Skip = v
		case "snapshot":
			s.Snapshot, _ = strconv.ParseInt(v, 10, 64)
		case "rollback":
			s.NoRollback = v == "off"
		}
	}
	if sc.Err() != nil {
		return State{}
	}
	return s
}

// Save writes the state (beside, then renamed: never half written).
func Save(path string, s State) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		return err
	}
	rollback := "on"
	if s.NoRollback {
		rollback = "off"
	}
	text := fmt.Sprintf("current=%s\npending=%s\nattempts=%d\nfailed=%s\nreason=%s\nskip=%s\nsnapshot=%d\nrollback=%s\n",
		s.Current, s.Pending, s.Attempts, s.Failed, strings.ReplaceAll(s.Reason, "\n", " "), s.Skip, s.Snapshot, rollback)
	tmp := path + ".tmp"
	if err := os.WriteFile(tmp, []byte(text), 0o600); err != nil {
		return err
	}
	return os.Rename(tmp, path)
}

// ---- deciding ----------------------------------------------------------------------

// Plan is what this start runs: Version, and whether it's a Trial (health
// checked; on failure, Fallback runs instead).
type Plan struct {
	Version  string
	Trial    bool
	Fallback string
}

// Decide picks the version from what's installed (sorted oldest first, by
// `newer`) and the state, and returns the state to save before starting.
func Decide(s State, installed []string, newer func(a, b string) bool) (Plan, State, error) {
	if len(installed) == 0 {
		return Plan{}, s, errors.New("no version of WhiteRose is installed")
	}
	sorted := append([]string(nil), installed...)
	sort.Slice(sorted, func(i, j int) bool { return newer(sorted[j], sorted[i]) })
	newest := sorted[len(sorted)-1]

	// The working version, if it's still there; else the newest that hasn't failed.
	if !slices.Contains(sorted, s.Current) {
		s.Current = ""
		for _, v := range slices.Backward(sorted) {
			if v != s.Failed {
				s.Current = v
				break
			}
		}
		if s.Current == "" {
			s.Current = newest
		}
	}
	// A pending one that's gone (removed by hand): forgotten.
	if s.Pending != "" && (!slices.Contains(sorted, s.Pending) || s.Pending == s.Current) {
		s.Pending, s.Attempts = "", 0
	}
	// A newer one installed (the Updates page, or apt / dnf): to try, unless
	// it already failed, or you chose to stay on (or roll back from) it. It
	// replaces a pending one it's newer than (that one may be the reason a
	// fix came out), and a failure of an older one is no longer worth asking
	// about.
	if newer(newest, s.Current) && newest != s.Failed && newest != s.Skip &&
		(s.Pending == "" || newer(newest, s.Pending)) {
		s.Pending, s.Attempts = newest, 0
		if s.Failed != "" && newer(newest, s.Failed) {
			s.Failed, s.Reason = "", ""
		}
	}
	if s.Pending != "" && s.NoRollback {
		// No health check: it's the one from now on.
		s = Passed(s, s.Pending)
		return Plan{Version: s.Current}, s, nil
	}
	if s.Pending != "" && s.Attempts < Tries {
		s.Attempts++
		return Plan{Version: s.Pending, Trial: true, Fallback: s.Current}, s, nil
	}
	return Plan{Version: s.Current}, s, nil
}

// Passed: the tried version is the one from now on.
func Passed(s State, version string) State {
	s.Current, s.Pending, s.Attempts = version, "", 0
	if s.Failed == version {
		s.Failed, s.Reason = "", ""
	}
	if s.Skip == version {
		s.Skip = ""
	}
	return s
}

// FailedTry: noted for the prompt (the version before runs instead).
func FailedTry(s State, version, reason string) State {
	s.Failed, s.Reason = version, reason
	return s
}

// TryAgain (the prompt): the failed version gets two more tries.
func TryAgain(s State) State {
	if s.Failed != "" {
		s.Pending, s.Attempts = s.Failed, 0
		if s.Skip == s.Failed {
			s.Skip = ""
		}
		s.Failed, s.Reason = "", ""
	}
	return s
}

// Stay (the prompt): keep the working version; the failed one isn't tried
// again (until a newer one comes), and the prompt goes away.
func Stay(s State) State {
	if s.Failed != "" {
		s.Skip = s.Failed
	} else if s.Pending != "" {
		s.Skip = s.Pending
	}
	s.Pending, s.Attempts, s.Failed, s.Reason = "", 0, "", ""
	return s
}

// Apply (Updates › Apply at next login): `version` is started (and health
// checked) at the next session start.
func Apply(s State, version string) State {
	if version == s.Current {
		return s
	}
	s.Pending, s.Attempts = version, 0
	if s.Skip == version {
		s.Skip = ""
	}
	if s.Failed == version {
		s.Failed, s.Reason = "", ""
	}
	return s
}

// Rollback (Updates › Roll back): `to` from the next session start; the
// version left isn't tried again until a newer one comes.
func Rollback(s State, to string) State {
	if to == s.Current {
		return s
	}
	s.Skip = s.Current
	s.Current, s.Pending, s.Attempts, s.Failed, s.Reason = to, "", 0, "", ""
	return s
}

// ---- settings snapshot ------------------------------------------------------------------

// What's snapshotted before a try: WhiteRose's config folder and its
// settings database (relative to home).
var snapshotItems = []string{
	".config/whiterose",
	".local/share/whiterose/settings.db",
	".local/share/whiterose/settings.db-wal",
	".local/share/whiterose/settings.db-shm",
}

// Snapshot copies them into `dir` (replacing the snapshot before: one only).
func Snapshot(home, dir string) error {
	tmp := dir + ".new"
	os.RemoveAll(tmp)
	if err := os.MkdirAll(tmp, 0o700); err != nil {
		return err
	}
	for _, it := range snapshotItems {
		src := filepath.Join(home, it)
		if _, err := os.Lstat(src); err != nil {
			continue
		}
		if err := copyTree(src, filepath.Join(tmp, it)); err != nil {
			os.RemoveAll(tmp)
			return err
		}
	}
	os.RemoveAll(dir)
	return os.Rename(tmp, dir)
}

// Restore puts the snapshot back (what it holds replaces what's there;
// what it lacks is removed, as it didn't exist then). What it replaces is
// kept in ReplacedDir(dir), the last time's only: settings changed while
// the failed version ran (a calendar added) aren't lost for good.
func Restore(home, dir string) error {
	if _, err := os.Stat(dir); err != nil {
		return errors.New("no snapshot")
	}
	aside := ReplacedDir(dir)
	os.RemoveAll(aside)
	for _, it := range snapshotItems {
		dst := filepath.Join(home, it)
		src := filepath.Join(dir, it)
		if _, err := os.Lstat(dst); err == nil {
			to := filepath.Join(aside, it)
			if os.MkdirAll(filepath.Dir(to), 0o700) != nil || os.Rename(dst, to) != nil {
				if err := os.RemoveAll(dst); err != nil {
					return err
				}
			}
		}
		if _, err := os.Lstat(src); err != nil {
			continue
		}
		if err := copyTree(src, dst); err != nil {
			return err
		}
	}
	return nil
}

// ReplacedDir is where Restore keeps what it replaced.
func ReplacedDir(snapshot string) string { return snapshot + "-replaced" }

func copyTree(src, dst string) error {
	return filepath.Walk(src, func(p string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		rel, _ := filepath.Rel(src, p)
		to := filepath.Join(dst, rel)
		switch {
		case info.IsDir():
			return os.MkdirAll(to, info.Mode().Perm()|0o700)
		case info.Mode()&os.ModeSymlink != 0:
			target, err := os.Readlink(p)
			if err != nil {
				return err
			}
			os.MkdirAll(filepath.Dir(to), 0o700)
			return os.Symlink(target, to)
		case info.Mode().IsRegular():
			if err := os.MkdirAll(filepath.Dir(to), 0o700); err != nil {
				return err
			}
			in, err := os.Open(p)
			if err != nil {
				return err
			}
			defer in.Close()
			out, err := os.OpenFile(to, os.O_CREATE|os.O_TRUNC|os.O_WRONLY, info.Mode().Perm())
			if err != nil {
				return err
			}
			if _, err := io.Copy(out, in); err != nil {
				out.Close()
				return err
			}
			return out.Close()
		}
		return nil
	})
}
