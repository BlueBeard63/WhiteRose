package main

import (
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"whiterose/daemon/internal/update"
)

// releaseKeyFile is where the release key's private half lives: on the
// machine that makes releases, never in the repository.
func releaseKeyFile() string {
	dir := os.Getenv("XDG_CONFIG_HOME")
	if dir == "" {
		dir = filepath.Join(os.Getenv("HOME"), ".config")
	}
	return filepath.Join(dir, "whiterose-release", "release.key")
}

// whiterosed release-key PUBLIC_OUT: makes the key pair (once).
func cmdReleaseKey(args []string, stdout, stderr io.Writer) int {
	if len(args) != 1 {
		fmt.Fprintln(stderr, "usage: whiterosed release-key PUBLIC_KEY_FILE")
		return 2
	}
	keyFile := releaseKeyFile()
	if _, err := os.Stat(keyFile); err == nil {
		fmt.Fprintf(stderr, "whiterosed: %s already exists (one key signs every release)\n", keyFile)
		return 1
	}
	priv, pub, err := update.NewKey()
	if err != nil {
		fmt.Fprintln(stderr, "whiterosed:", err)
		return 1
	}
	if err := os.MkdirAll(filepath.Dir(keyFile), 0o700); err != nil {
		fmt.Fprintln(stderr, "whiterosed:", err)
		return 1
	}
	if err := os.WriteFile(keyFile, []byte(priv+"\n"), 0o600); err != nil {
		fmt.Fprintln(stderr, "whiterosed:", err)
		return 1
	}
	if err := os.WriteFile(args[0], []byte(pub+"\n"), 0o644); err != nil {
		fmt.Fprintln(stderr, "whiterosed:", err)
		return 1
	}
	fmt.Fprintf(stdout, "Release key %s\n  private: %s (back it up; keep it secret)\n  public:  %s (commit this)\n", update.KeyID(pub), keyFile, args[0])
	return 0
}

// whiterosed sign-release DIR PUBLIC_KEY_FILE: SHA256SUMS for the packages
// in DIR, and its signature, checked against the public key before use.
func cmdSignRelease(args []string, stdout, stderr io.Writer) int {
	if len(args) != 2 {
		fmt.Fprintln(stderr, "usage: whiterosed sign-release DIR PUBLIC_KEY_FILE")
		return 2
	}
	dir := args[0]
	priv, err := os.ReadFile(releaseKeyFile())
	if err != nil {
		fmt.Fprintln(stderr, "whiterosed: no release key here (make release-key)")
		return 1
	}
	pub, err := os.ReadFile(args[1])
	if err != nil {
		fmt.Fprintln(stderr, "whiterosed:", err)
		return 1
	}
	entries, err := os.ReadDir(dir)
	if err != nil {
		fmt.Fprintln(stderr, "whiterosed:", err)
		return 1
	}
	var names []string
	for _, e := range entries {
		n := e.Name()
		if strings.HasSuffix(n, ".deb") || strings.HasSuffix(n, ".rpm") {
			names = append(names, n)
		}
	}
	if len(names) == 0 {
		fmt.Fprintln(stderr, "whiterosed: no packages in", dir)
		return 1
	}
	sums, err := update.MakeSums(dir, names)
	// Each package's build ID (build.sh writes build-<version>-<kind>.id):
	// signed with the packages, so an installed WhiteRose can show it's
	// this release, unchanged (bug reports need that).
	if err == nil {
		var ids []byte
		ids, err = buildLines(dir)
		sums = append(sums, ids...)
	}
	if err == nil {
		var sig string
		sig, err = update.Sign(string(priv), sums)
		if err == nil {
			// The private key must match the public key WhiteRose ships.
			if err = update.Verify(string(pub), sums, sig); err == nil {
				err = os.WriteFile(filepath.Join(dir, "SHA256SUMS"), sums, 0o644)
			}
			if err == nil {
				err = os.WriteFile(filepath.Join(dir, "SHA256SUMS.sig"), []byte(sig+"\n"), 0o644)
			}
		}
	}
	if err != nil {
		fmt.Fprintln(stderr, "whiterosed:", err)
		return 1
	}
	fmt.Fprintf(stdout, "Signed %d packages and their build IDs with release key %s\n", len(names), update.KeyID(string(pub)))
	return 0
}

// buildLines: "<id>  build-<version>-<kind>" for each .id file in dir.
func buildLines(dir string) ([]byte, error) {
	ids, _ := filepath.Glob(filepath.Join(dir, "build-*.id"))
	if len(ids) == 0 {
		return nil, errors.New("no build IDs with the packages (build-<version>-<kind>.id): make package writes them")
	}
	sort.Strings(ids)
	var out []byte
	for _, p := range ids {
		b, err := os.ReadFile(p)
		if err != nil {
			return nil, err
		}
		id := strings.TrimSpace(string(b))
		if _, err := hex.DecodeString(id); err != nil || len(id) != 64 {
			return nil, fmt.Errorf("%s doesn't hold a build ID", filepath.Base(p))
		}
		out = append(out, id+"  "+strings.TrimSuffix(filepath.Base(p), ".id")+"\n"...)
	}
	return out, nil
}

// whiterosed build-id DIR: a version folder's build ID (build.sh, after
// laying out /usr/lib/whiterose/<version>).
func cmdBuildID(args []string, stdout, stderr io.Writer) int {
	if len(args) != 1 {
		fmt.Fprintln(stderr, "usage: whiterosed build-id DIR")
		return 2
	}
	id, err := update.BuildID(args[0])
	if err != nil {
		fmt.Fprintln(stderr, "whiterosed:", err)
		return 1
	}
	fmt.Fprintln(stdout, id)
	return 0
}
