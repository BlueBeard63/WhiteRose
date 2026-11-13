package update

import (
	"os"
	"path/filepath"
	"slices"
	"sort"
	"strings"
)

// Root is where versions are installed, each in its own folder.
const Root = "/usr/lib/whiterose"

// validVersion: digits, dots, letters, - and ~, starting with a digit.
func validVersion(v string) bool {
	if v == "" || v[0] < '0' || v[0] > '9' || len(v) > 40 {
		return false
	}
	return strings.Trim(v, "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ.-~") == ""
}

// Installed lists the versions in `root` (complete ones: with the
// compositor in them), oldest first.
func Installed(root string) []string {
	entries, err := os.ReadDir(root)
	if err != nil {
		return nil
	}
	var v []string
	for _, e := range entries {
		n := e.Name()
		if !e.IsDir() || !validVersion(n) {
			continue
		}
		if st, err := os.Stat(filepath.Join(root, n, "bin", "whiterose-wm")); err == nil && st.Mode()&0o111 != 0 {
			v = append(v, n)
		}
	}
	sort.Slice(v, func(i, j int) bool { return compare(v[i], v[j]) < 0 })
	return v
}

// Keep is what stays installed: the newest version, and the one being run
// (if it's another), or else the one before the newest, to go back to.
// Never more than two: old versions don't pile up. `running` may be "".
//
// `chosen`: a version picked under Settings › Updates › Releases (older
// ones too): kept as well, so three at most. The newest always stays: the
// whiterose package (the session, the commands) depends on it.
func Keep(installed []string, running string, chosen ...string) []string {
	if len(installed) == 0 {
		return nil
	}
	sorted := append([]string(nil), installed...)
	sort.Slice(sorted, func(i, j int) bool { return compare(sorted[i], sorted[j]) < 0 })
	newest := sorted[len(sorted)-1]
	keep := []string{newest}
	has := func(v string) bool {
		for _, x := range sorted {
			if x == v {
				return true
			}
		}
		return false
	}
	switch {
	case running != "" && running != newest && has(running):
		keep = append(keep, running)
	case len(sorted) > 1:
		keep = append(keep, sorted[len(sorted)-2])
	}
	for _, c := range chosen {
		if c != "" && has(c) && !slices.Contains(keep, c) {
			keep = append(keep, c)
		}
	}
	return keep
}

// Remove is what Keep leaves out (to uninstall), oldest first.
func Remove(installed []string, running string, chosen ...string) []string {
	keep := Keep(installed, running, chosen...)
	var out []string
	for _, v := range installed {
		kept := false
		for _, k := range keep {
			kept = kept || k == v
		}
		if !kept {
			out = append(out, v)
		}
	}
	sort.Slice(out, func(i, j int) bool { return compare(out[i], out[j]) < 0 })
	return out
}

// CorePackage is a version's package name.
func CorePackage(version string) string { return "whiterose-core-" + version }
