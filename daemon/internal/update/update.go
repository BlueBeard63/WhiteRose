// Package update finds, downloads and verifies WhiteRose updates.
//
// A release on GitHub carries the packages (.deb and .rpm) and two small
// files: SHA256SUMS (a checksum per file) and SHA256SUMS.sig, an ed25519
// signature of SHA256SUMS made with WhiteRose's release key. The public
// half ships with WhiteRose (/usr/share/whiterose/release.pub), so an update
// is only used when:
//
//  1. downloaded: the package for this computer, completely;
//  2. signature: SHA256SUMS was signed by the release key;
//  3. files verified: the package's SHA-256 matches its line in SHA256SUMS.
//
// Nothing is installed until all three pass.
package update

import (
	"bufio"
	"bytes"
	"context"
	"crypto/ed25519"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"time"
)

// ---- keys and signatures ----------------------------------------------------------

// NewKey makes a release key pair: the private key (keep it secret, off the
// repository) and the public key (shipped), both base64.
func NewKey() (private, public string, err error) {
	pub, priv, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		return "", "", err
	}
	return base64.StdEncoding.EncodeToString(priv.Seed()), base64.StdEncoding.EncodeToString(pub), nil
}

func decodeKey(text string, size int) ([]byte, error) {
	b, err := base64.StdEncoding.DecodeString(strings.TrimSpace(text))
	if err != nil || len(b) != size {
		return nil, errors.New("not a release key")
	}
	return b, nil
}

// Sign signs `data` with a private key (from NewKey): the signature, base64.
func Sign(private string, data []byte) (string, error) {
	seed, err := decodeKey(private, ed25519.SeedSize)
	if err != nil {
		return "", err
	}
	return base64.StdEncoding.EncodeToString(ed25519.Sign(ed25519.NewKeyFromSeed(seed), data)), nil
}

// Verify checks `sig` (base64) over `data` against a public key.
func Verify(public string, data []byte, sig string) error {
	pub, err := decodeKey(public, ed25519.PublicKeySize)
	if err != nil {
		return err
	}
	s, err := base64.StdEncoding.DecodeString(strings.TrimSpace(sig))
	if err != nil || !ed25519.Verify(pub, data, s) {
		return errors.New("the signature doesn't match WhiteRose's release key")
	}
	return nil
}

// KeyID is a short fingerprint of a public key, to show ("7F3A…C21E").
func KeyID(public string) string {
	h := sha256.Sum256([]byte(strings.TrimSpace(public)))
	x := strings.ToUpper(hex.EncodeToString(h[:]))
	return x[:4] + "…" + x[len(x)-4:]
}

// ---- SHA256SUMS -------------------------------------------------------------------------

// Sums reads SHA256SUMS ("<hex>  <name>" per line): name → hex.
func Sums(text []byte) (map[string]string, error) {
	out := map[string]string{}
	sc := bufio.NewScanner(bytes.NewReader(text))
	for sc.Scan() {
		line := strings.TrimSpace(sc.Text())
		if line == "" {
			continue
		}
		f := strings.Fields(line)
		if len(f) != 2 || len(f[0]) != 64 {
			return nil, fmt.Errorf("SHA256SUMS: bad line %q", line)
		}
		if _, err := hex.DecodeString(f[0]); err != nil {
			return nil, fmt.Errorf("SHA256SUMS: bad line %q", line)
		}
		out[strings.TrimPrefix(f[1], "*")] = strings.ToLower(f[0])
	}
	return out, sc.Err()
}

// MakeSums writes SHA256SUMS for the files in `dir` (sorted by name).
func MakeSums(dir string, names []string) ([]byte, error) {
	sort.Strings(names)
	var b bytes.Buffer
	for _, n := range names {
		h, err := fileSum(filepath.Join(dir, n))
		if err != nil {
			return nil, err
		}
		fmt.Fprintf(&b, "%s  %s\n", h, n)
	}
	return b.Bytes(), nil
}

func fileSum(path string) (string, error) {
	f, err := os.Open(path)
	if err != nil {
		return "", err
	}
	defer f.Close()
	h := sha256.New()
	if _, err := io.Copy(h, f); err != nil {
		return "", err
	}
	return hex.EncodeToString(h.Sum(nil)), nil
}

// ---- versions ------------------------------------------------------------------------------

// Newer reports whether version a is newer than b ("v0.5.0" > "0.4.2";
// "0.5.0" > "0.5.0-rc1").
func Newer(a, b string) bool {
	return compare(a, b) > 0
}

func compare(a, b string) int {
	split := func(v string) ([]int, string) {
		v = strings.TrimPrefix(strings.TrimSpace(v), "v")
		pre := ""
		if i := strings.IndexAny(v, "-~"); i >= 0 {
			v, pre = v[:i], v[i+1:]
		}
		var nums []int
		for p := range strings.SplitSeq(v, ".") {
			n, _ := strconv.Atoi(p)
			nums = append(nums, n)
		}
		return nums, pre
	}
	an, ap := split(a)
	bn, bp := split(b)
	for i := 0; i < len(an) || i < len(bn); i++ {
		x, y := 0, 0
		if i < len(an) {
			x = an[i]
		}
		if i < len(bn) {
			y = bn[i]
		}
		if x != y {
			if x > y {
				return 1
			}
			return -1
		}
	}
	switch {
	case ap == bp:
		return 0
	case ap == "": // a release beats its pre-releases
		return 1
	case bp == "":
		return -1
	case ap > bp:
		return 1
	default:
		return -1
	}
}

// ---- releases -------------------------------------------------------------------------------

type Asset struct {
	Name string `json:"name"`
	Size int64  `json:"size"`
	URL  string `json:"browser_download_url"`
}

type Release struct {
	Tag       string    `json:"tag_name"`
	Notes     string    `json:"body"`
	Published time.Time `json:"published_at"`
	Draft     bool      `json:"draft"`
	Pre       bool      `json:"prerelease"`
	Assets    []Asset   `json:"assets"`
}

// Version is the release's version ("0.5.0" from tag "v0.5.0").
func (r Release) Version() string { return strings.TrimPrefix(r.Tag, "v") }

// Changes are the release notes' bullet points (up to 6).
func (r Release) Changes() []string {
	var out []string
	for l := range strings.SplitSeq(r.Notes, "\n") {
		l = strings.TrimSpace(l)
		for _, p := range []string{"- ", "* ", "• "} {
			if strings.HasPrefix(l, p) {
				out = append(out, strings.TrimSpace(l[len(p):]))
				break
			}
		}
		if len(out) == 6 {
			break
		}
	}
	return out
}

// Packages are the release's packages for this kind of system ("deb" or
// "rpm"): whiterose-core-<version> and whiterose (installed together).
func (r Release) Packages(kind string) []Asset {
	var out []Asset
	for _, a := range r.Assets {
		if (kind == "deb" && strings.HasSuffix(a.Name, "_amd64.deb")) || (kind == "rpm" && strings.HasSuffix(a.Name, ".x86_64.rpm")) {
			out = append(out, a)
		}
	}
	return out
}

func (r Release) asset(name string) (Asset, bool) {
	for _, a := range r.Assets {
		if a.Name == name {
			return a, true
		}
	}
	return Asset{}, false
}

// Source is where releases come from: a GitHub repository ("owner/name").
type Source struct {
	Repo string
	// The API's address (tests point it elsewhere).
	API    string
	Client *http.Client
}

func (s Source) client() *http.Client {
	if s.Client != nil {
		return s.Client
	}
	return &http.Client{Timeout: 30 * time.Second}
}

// Latest is the newest published release (not drafts or pre-releases).
func (s Source) Latest(ctx context.Context) (Release, error) {
	if !validRepo(s.Repo) {
		return Release{}, errors.New("no update source set")
	}
	api := s.API
	if api == "" {
		api = "https://api.github.com"
	}
	req, _ := http.NewRequestWithContext(ctx, "GET", api+"/repos/"+s.Repo+"/releases/latest", nil)
	req.Header.Set("Accept", "application/vnd.github+json")
	req.Header.Set("User-Agent", "whiterosed")
	res, err := s.client().Do(req)
	if err != nil {
		return Release{}, fmt.Errorf("couldn't reach the update server: %w", err)
	}
	defer res.Body.Close()
	if res.StatusCode == http.StatusNotFound {
		return Release{}, errors.New("there are no releases yet")
	}
	if res.StatusCode != http.StatusOK {
		return Release{}, fmt.Errorf("the update server said %s", res.Status)
	}
	var r Release
	if err := json.NewDecoder(io.LimitReader(res.Body, 4<<20)).Decode(&r); err != nil {
		return Release{}, fmt.Errorf("the update server's answer was garbled: %w", err)
	}
	return r, nil
}

func validRepo(r string) bool {
	parts := strings.Split(r, "/")
	if len(parts) != 2 {
		return false
	}
	for _, p := range parts {
		if p == "" || len(p) > 100 || strings.Trim(p, "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_.") != "" {
			return false
		}
	}
	return true
}

// ---- downloading and verifying ---------------------------------------------------------

// Step is where a download has got to, for the page's steps.
type Step struct {
	Name   string `json:"name"`   // downloaded | signature | verified
	Done   bool   `json:"done"`   // passed
	Detail string `json:"detail"` // "48.2 MB, complete", "Release key 7F3A…C21E"
}

// Progress reports bytes downloaded of the whole.
type Progress func(done, total int64)

func (s Source) fetch(ctx context.Context, url string, dst io.Writer, max int64, prog Progress, total int64) error {
	req, _ := http.NewRequestWithContext(ctx, "GET", url, nil)
	req.Header.Set("User-Agent", "whiterosed")
	res, err := s.client().Do(req)
	if err != nil {
		return err
	}
	defer res.Body.Close()
	if res.StatusCode != http.StatusOK {
		return fmt.Errorf("download failed: %s", res.Status)
	}
	var done int64
	buf := make([]byte, 64<<10)
	for {
		n, err := res.Body.Read(buf)
		if n > 0 {
			done += int64(n)
			if done > max {
				return errors.New("the download is bigger than it should be")
			}
			if _, werr := dst.Write(buf[:n]); werr != nil {
				return werr
			}
			if prog != nil {
				prog(done, total)
			}
		}
		if err == io.EOF {
			return nil
		}
		if err != nil {
			return err
		}
	}
}

// Fetch downloads the release's packages for `kind` into `dir` and
// verifies them against SHA256SUMS and its signature (by `public`). On
// success: the packages' paths and the steps; on failure the steps so far,
// and nothing downloaded is kept.
func (s Source) Fetch(ctx context.Context, r Release, kind, public, dir string, prog Progress) ([]string, []Step, error) {
	steps := []Step{{Name: "downloaded"}, {Name: "signature"}, {Name: "verified"}}
	pkgs := r.Packages(kind)
	if len(pkgs) == 0 {
		return nil, steps, fmt.Errorf("this release has no .%s packages", kind)
	}
	sumsA, ok1 := r.asset("SHA256SUMS")
	sigA, ok2 := r.asset("SHA256SUMS.sig")
	if !ok1 || !ok2 {
		return nil, steps, errors.New("this release isn't signed")
	}
	if err := os.MkdirAll(dir, 0o700); err != nil {
		return nil, steps, err
	}
	// The small files first.
	var sums, sig bytes.Buffer
	if err := s.fetch(ctx, sumsA.URL, &sums, 1<<20, nil, 0); err != nil {
		return nil, steps, err
	}
	if err := s.fetch(ctx, sigA.URL, &sig, 4<<10, nil, 0); err != nil {
		return nil, steps, err
	}
	// The packages, under temporary names until they check out.
	var total, before int64
	for _, p := range pkgs {
		total += p.Size
	}
	var parts []string
	fail := func(e error) ([]string, []Step, error) {
		for _, p := range parts {
			os.Remove(p)
		}
		return nil, steps, e
	}
	for _, pkg := range pkgs {
		part := filepath.Join(dir, filepath.Base(pkg.Name)) + ".part"
		parts = append(parts, part)
		f, err := os.OpenFile(part, os.O_CREATE|os.O_TRUNC|os.O_WRONLY, 0o600)
		if err != nil {
			return fail(err)
		}
		b := before
		err = s.fetch(ctx, pkg.URL, f, pkg.Size+1, func(d, _ int64) {
			if prog != nil {
				prog(b+d, total)
			}
		}, pkg.Size)
		f.Close()
		if err != nil {
			return fail(err)
		}
		st, _ := os.Stat(part)
		if st == nil || (pkg.Size > 0 && st.Size() != pkg.Size) {
			return fail(errors.New("the download was cut short"))
		}
		before += st.Size()
	}
	steps[0] = Step{Name: "downloaded", Done: true, Detail: fmt.Sprintf("%s, complete", Size(before))}

	if err := Verify(public, sums.Bytes(), sig.String()); err != nil {
		return fail(err)
	}
	steps[1] = Step{Name: "signature", Done: true, Detail: "Release key " + KeyID(public)}

	table, err := Sums(sums.Bytes())
	if err != nil {
		return fail(err)
	}
	for i, pkg := range pkgs {
		want, ok := table[pkg.Name]
		if !ok {
			return fail(fmt.Errorf("%s isn't in the signed checksums", pkg.Name))
		}
		got, err := fileSum(parts[i])
		if err != nil {
			return fail(err)
		}
		if got != want {
			return fail(fmt.Errorf("%s doesn't match its signed checksum", pkg.Name))
		}
	}
	steps[2] = Step{Name: "verified", Done: true, Detail: fmt.Sprintf("%d packages · sha256", len(pkgs))}
	// Kept beside them: installing checks them again, as root.
	if err := os.WriteFile(filepath.Join(dir, "SHA256SUMS"), sums.Bytes(), 0o600); err != nil {
		return fail(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "SHA256SUMS.sig"), sig.Bytes(), 0o600); err != nil {
		return fail(err)
	}
	var paths []string
	for _, part := range parts {
		path := strings.TrimSuffix(part, ".part")
		if err := os.Rename(part, path); err != nil {
			return fail(err)
		}
		paths = append(paths, path)
	}
	return paths, steps, nil
}

// Size is bytes for people ("48.2 MB").
func Size(n int64) string {
	switch {
	case n >= 1e9:
		return fmt.Sprintf("%.1f GB", float64(n)/1e9)
	case n >= 1e6:
		return fmt.Sprintf("%.1f MB", float64(n)/1e6)
	case n >= 1e3:
		return fmt.Sprintf("%.0f kB", float64(n)/1e3)
	}
	return fmt.Sprintf("%d B", n)
}

// PackageName says whether `name` is a WhiteRose package file of `kind`
// ("whiterose_0.5.0_amd64.deb", "whiterose-core-0.5.0-0.5.0-1.x86_64.rpm").
func PackageName(name, kind string) bool {
	if !strings.HasPrefix(name, "whiterose") || strings.ContainsAny(name, "/\\ ") || strings.HasPrefix(name, ".") {
		return false
	}
	switch kind {
	case "deb":
		return strings.HasSuffix(name, "_amd64.deb")
	case "rpm":
		return strings.HasSuffix(name, ".x86_64.rpm")
	}
	return false
}

// VerifyDir checks a downloaded release in `dir` (what Fetch left): the
// checksums' signature by `public`, and every package of `kind` there
// against them. Returns the packages' paths. Installing runs it again as
// root, on its own copy, so nothing swapped in after the download counts.
func VerifyDir(dir, kind, public string) ([]string, error) {
	sums, err := os.ReadFile(filepath.Join(dir, "SHA256SUMS"))
	if err != nil {
		return nil, errors.New("no checksums with the download")
	}
	sig, err := os.ReadFile(filepath.Join(dir, "SHA256SUMS.sig"))
	if err != nil {
		return nil, errors.New("no signature with the download")
	}
	if err := Verify(public, sums, string(sig)); err != nil {
		return nil, err
	}
	table, err := Sums(sums)
	if err != nil {
		return nil, err
	}
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil, err
	}
	var paths []string
	for _, e := range entries {
		if !e.Type().IsRegular() || !PackageName(e.Name(), kind) {
			continue
		}
		want, ok := table[e.Name()]
		if !ok {
			return nil, fmt.Errorf("%s isn't in the signed checksums", e.Name())
		}
		p := filepath.Join(dir, e.Name())
		got, err := fileSum(p)
		if err != nil {
			return nil, err
		}
		if got != want {
			return nil, fmt.Errorf("%s doesn't match its signed checksum", e.Name())
		}
		paths = append(paths, p)
	}
	if len(paths) == 0 {
		return nil, fmt.Errorf("no .%s packages in the download", kind)
	}
	return paths, nil
}
