package update

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestSignAndVerify(t *testing.T) {
	priv, pub, err := NewKey()
	if err != nil {
		t.Fatal(err)
	}
	sig, err := Sign(priv, []byte("hello"))
	if err != nil {
		t.Fatal(err)
	}
	if err := Verify(pub, []byte("hello"), sig); err != nil {
		t.Fatalf("good signature refused: %v", err)
	}
	if Verify(pub, []byte("hellp"), sig) == nil {
		t.Error("changed data accepted")
	}
	_, other, _ := NewKey()
	if Verify(other, []byte("hello"), sig) == nil {
		t.Error("another key accepted")
	}
	if Verify("not a key", []byte("hello"), sig) == nil || Verify(pub, []byte("hello"), "garbage!") == nil {
		t.Error("garbage accepted")
	}
	if id := KeyID(pub); len([]rune(id)) != 9 {
		t.Errorf("key id %q", id)
	}
}

func TestSumsAndVersions(t *testing.T) {
	m, err := Sums([]byte(strings.Repeat("a", 64) + "  pkg.deb\n" + strings.Repeat("B", 64) + " *pkg.rpm\n\n"))
	if err != nil || m["pkg.deb"] != strings.Repeat("a", 64) || m["pkg.rpm"] != strings.Repeat("b", 64) {
		t.Fatalf("%v %v", m, err)
	}
	if _, err := Sums([]byte("short  x\n")); err == nil {
		t.Error("bad line accepted")
	}
	cases := []struct {
		a, b  string
		newer bool
	}{
		{"v0.5.0", "0.4.2", true}, {"0.4.10", "0.4.9", true}, {"0.4.2", "0.4.2", false},
		{"0.5.0", "0.5.0-rc1", true}, {"0.5.0-rc2", "0.5.0-rc1", true}, {"0.4.9", "0.5.0", false}, {"1.0", "0.9.9", true},
	}
	for _, c := range cases {
		if Newer(c.a, c.b) != c.newer {
			t.Errorf("Newer(%s, %s) != %v", c.a, c.b, c.newer)
		}
	}
	r := Release{Notes: "## 0.5.0\n- New window switcher\n* Drag to arrange\nnot a bullet\n• Bluetooth fix"}
	if got := r.Changes(); len(got) != 3 || got[2] != "Bluetooth fix" {
		t.Errorf("changes %v", got)
	}
}

// A fake GitHub with one release whose files can be tampered with.
func fakeRelease(t *testing.T, pkg []byte, sums func([]byte) []byte, sig func(string) string, priv string) (*httptest.Server, Source) {
	t.Helper()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "whiterose_0.5.0_amd64.deb"), pkg, 0o644)
	s, _ := MakeSums(dir, []string{"whiterose_0.5.0_amd64.deb"})
	signed, _ := Sign(priv, s)
	s = sums(s)
	files := map[string][]byte{
		"whiterose_0.5.0_amd64.deb": pkg,
		"SHA256SUMS":                s,
		"SHA256SUMS.sig":            []byte(sig(signed)),
	}
	var srv *httptest.Server
	srv = httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/repos/me/whiterose/releases/latest" {
			var assets []Asset
			for n, b := range files {
				assets = append(assets, Asset{Name: n, Size: int64(len(b)), URL: srv.URL + "/dl/" + n})
			}
			json.NewEncoder(w).Encode(Release{Tag: "v0.5.0", Notes: "- Faster", Assets: assets})
			return
		}
		if b, ok := files[strings.TrimPrefix(r.URL.Path, "/dl/")]; ok {
			w.Write(b)
			return
		}
		http.NotFound(w, r)
	}))
	t.Cleanup(srv.Close)
	return srv, Source{Repo: "me/whiterose", API: srv.URL}
}

func TestFetchVerifies(t *testing.T) {
	priv, pub, _ := NewKey()
	same := func(b []byte) []byte { return b }
	keep := func(s string) string { return s }
	ctx := context.Background()

	_, src := fakeRelease(t, []byte("the package"), same, keep, priv)
	r, err := src.Latest(ctx)
	if err != nil || r.Version() != "0.5.0" {
		t.Fatalf("%v %v", r, err)
	}
	dir := t.TempDir()
	var seen int64
	paths, steps, err := src.Fetch(ctx, r, "deb", pub, dir, func(d, _ int64) { seen = d })
	if err != nil || len(paths) != 1 {
		t.Fatalf("good release refused: %v", err)
	}
	if b, _ := os.ReadFile(paths[0]); string(b) != "the package" || seen != 11 {
		t.Errorf("got %q, progress %d", b, seen)
	}
	for _, s := range steps {
		if !s.Done {
			t.Errorf("step %s not done", s.Name)
		}
	}
	// Checked again from what's on disk (as installing does)...
	if got, err := VerifyDir(dir, "deb", pub); err != nil || len(got) != 1 {
		t.Errorf("verify dir: %v %v", got, err)
	}
	// ...and a package swapped afterwards is caught.
	os.WriteFile(paths[0], []byte("swapped"), 0o600)
	if _, err := VerifyDir(dir, "deb", pub); err == nil {
		t.Error("swapped package passed")
	}
	if _, _, err := src.Fetch(ctx, r, "rpm", pub, dir, nil); err == nil {
		t.Error("no rpm in the release, yet fetched")
	}

	// A package swapped after signing.
	_, bad := fakeRelease(t, []byte("evil package"), func(b []byte) []byte {
		// The sums say what the real package was.
		return []byte(strings.Replace(string(b), string(b[:64]), strings.Repeat("0", 64), 1))
	}, keep, priv)
	r2, _ := bad.Latest(ctx)
	_, steps, err = bad.Fetch(ctx, r2, "deb", pub, t.TempDir(), nil)
	if err == nil || steps[1].Done {
		t.Errorf("changed sums passed the signature: %v %v", err, steps)
	}

	// Signed by someone else.
	other, _, _ := NewKey()
	_, forged := fakeRelease(t, []byte("the package"), same, keep, other)
	r3, _ := forged.Latest(ctx)
	d3 := t.TempDir()
	if _, _, err := forged.Fetch(ctx, r3, "deb", pub, d3, nil); err == nil || !strings.Contains(err.Error(), "release key") {
		t.Errorf("forged signature: %v", err)
	}
	if left, _ := os.ReadDir(d3); len(left) != 0 {
		// (nothing at all: not the checksums either)
		t.Errorf("a refused download was left behind: %v", left)
	}

	if _, err := (Source{Repo: "../evil"}).Latest(ctx); err == nil {
		t.Error("bad repo name used")
	}
}

func TestPackageName(t *testing.T) {
	for name, ok := range map[string]bool{
		"whiterose_0.5.0_amd64.deb":            true,
		"whiterose-core-0.5.0_0.5.0_amd64.deb": true,
		"evil_0.5.0_amd64.deb":                 false,
		"whiterose_0.5.0_amd64.deb.part":       false,
		"../whiterose_0.5.0_amd64.deb":         false,
		"whiterose 1_amd64.deb":                false,
	} {
		if PackageName(name, "deb") != ok {
			t.Errorf("%s: %v", name, !ok)
		}
	}
	if !PackageName("whiterose-0.5.0-1.x86_64.rpm", "rpm") || PackageName("whiterose-0.5.0-1.x86_64.rpm", "deb") {
		t.Error("rpm")
	}
}
