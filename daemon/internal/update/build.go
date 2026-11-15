package update

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

// BuildID is a version folder's fingerprint (/usr/lib/whiterose/<version>):
// sha256 of a sorted list of "path  sha256" for every file in it ("path ->
// target" for a link). `make release` puts each package's ID in the signed
// SHA256SUMS as "<id>  build-<version>-<deb|rpm>", so an installed
// WhiteRose can tell it's exactly what was released (a bug report is only
// sent to the repository when it is: rebuilt or patched builds, forks
// included, aren't).
func BuildID(dir string) (string, error) {
	var lines []string
	err := filepath.WalkDir(dir, func(p string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		rel, _ := filepath.Rel(dir, p)
		rel = filepath.ToSlash(rel)
		switch {
		case d.Type()&fs.ModeSymlink != 0:
			t, err := os.Readlink(p)
			if err != nil {
				return err
			}
			lines = append(lines, rel+" -> "+t)
		case d.Type().IsRegular():
			h, err := fileSum(p)
			if err != nil {
				return err
			}
			lines = append(lines, rel+"  "+h)
		}
		return nil
	})
	if err != nil {
		return "", err
	}
	if len(lines) == 0 {
		return "", errors.New("nothing in " + dir)
	}
	sort.Strings(lines)
	h := sha256.Sum256([]byte(strings.Join(lines, "\n") + "\n"))
	return hex.EncodeToString(h[:]), nil
}

// BuildName is a build's name in SHA256SUMS: "build-0.3.1-deb".
func BuildName(version, kind string) string { return "build-" + version + "-" + kind }

// Build states (VerifyBuild).
const (
	BuildVerified    = "verified"    // matches the signed release
	BuildModified    = "modified"    // the release lists another build
	BuildUnlisted    = "unlisted"    // released before build IDs, or not released here
	BuildDevelopment = "development" // not an installed version
	BuildUnknown     = "unknown"     // couldn't check (offline)
)

// VerifyBuild checks build `id` of `version` (kind deb or rpm) against that
// release's SHA256SUMS in s.Repo, signed by `public`.
func (s Source) VerifyBuild(ctx context.Context, version, kind, id, public string) (string, error) {
	var r Release
	if err := s.get(ctx, "/releases/tags/v"+version, &r); err != nil {
		if strings.Contains(err.Error(), "no releases") {
			return BuildUnlisted, fmt.Errorf("%s isn't a release of %s", version, s.Repo)
		}
		return BuildUnknown, err
	}
	sums, err := s.small(ctx, r, "SHA256SUMS")
	if err != nil {
		return BuildUnknown, err
	}
	sig, err := s.small(ctx, r, "SHA256SUMS.sig")
	if err != nil {
		return BuildUnknown, err
	}
	if err := Verify(public, sums, string(sig)); err != nil {
		// Signed by another key: this build's key isn't that repository's.
		return BuildModified, errors.New("the release's checksums aren't signed by this build's release key")
	}
	table, err := Sums(sums)
	if err != nil {
		return BuildUnknown, err
	}
	want, ok := table[BuildName(version, kind)]
	switch {
	case !ok:
		return BuildUnlisted, fmt.Errorf("%s was released before builds could be checked", version)
	case want != id:
		return BuildModified, errors.New("this build's files don't match the release")
	}
	return BuildVerified, nil
}

// small downloads one of r's small files (checksums, signature).
func (s Source) small(ctx context.Context, r Release, name string) ([]byte, error) {
	a, ok := r.asset(name)
	if !ok {
		return nil, fmt.Errorf("the release has no %s", name)
	}
	var b strings.Builder
	if err := s.fetch(ctx, a.URL, &b, 1<<20, nil, 0); err != nil {
		return nil, err
	}
	return []byte(b.String()), nil
}
