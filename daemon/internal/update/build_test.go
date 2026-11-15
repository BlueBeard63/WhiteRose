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

func TestBuildID(t *testing.T) {
	dir := t.TempDir()
	os.MkdirAll(filepath.Join(dir, "bin"), 0o755)
	os.WriteFile(filepath.Join(dir, "bin", "quickshell"), []byte("qs"), 0o755)
	os.Symlink("quickshell", filepath.Join(dir, "bin", "qs"))
	os.MkdirAll(filepath.Join(dir, "shell"), 0o755)
	os.WriteFile(filepath.Join(dir, "shell", "Bar.qml"), []byte("Item {}"), 0o644)
	a, err := BuildID(dir)
	if err != nil || len(a) != 64 {
		t.Fatalf("%q %v", a, err)
	}
	if b, _ := BuildID(dir); b != a {
		t.Error("not stable")
	}
	// Any change is another build: a file's content, a link's target, a new file.
	for name, change := range map[string]func(){
		"content": func() { os.WriteFile(filepath.Join(dir, "shell", "Bar.qml"), []byte("Item { }"), 0o644) },
		"link": func() {
			os.Remove(filepath.Join(dir, "bin", "qs"))
			os.Symlink("whiterose-wm", filepath.Join(dir, "bin", "qs"))
		},
		"added": func() { os.WriteFile(filepath.Join(dir, "shell", "Patch.qml"), nil, 0o644) },
	} {
		before, _ := BuildID(dir)
		change()
		if after, _ := BuildID(dir); after == before {
			t.Errorf("%s: same ID", name)
		}
	}
	if _, err := BuildID(t.TempDir()); err == nil {
		t.Error("an empty folder has an ID")
	}
}

// A fake GitHub with release v0.5.0 whose SHA256SUMS lists `lines`, signed
// by `priv`; `missing` makes the release not exist.
func buildServer(t *testing.T, priv, lines string, missing bool) Source {
	t.Helper()
	sig, _ := Sign(priv, []byte(lines))
	var srv *httptest.Server
	srv = httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/repos/me/whiterose/releases/tags/v0.5.0":
			if missing {
				http.NotFound(w, r)
				return
			}
			json.NewEncoder(w).Encode(Release{Tag: "v0.5.0", Assets: []Asset{
				{Name: "SHA256SUMS", URL: srv.URL + "/dl/sums"}, {Name: "SHA256SUMS.sig", URL: srv.URL + "/dl/sig"}}})
		case "/dl/sums":
			w.Write([]byte(lines))
		case "/dl/sig":
			w.Write([]byte(sig))
		default:
			http.NotFound(w, r)
		}
	}))
	t.Cleanup(srv.Close)
	return Source{Repo: "me/whiterose", API: srv.URL}
}

func TestVerifyBuild(t *testing.T) {
	priv, pub, _ := NewKey()
	otherPriv, _, _ := NewKey()
	id := strings.Repeat("ab", 32)
	pkg := strings.Repeat("cd", 32) + "  whiterose_0.5.0_amd64.deb\n"
	ctx := context.Background()
	for name, c := range map[string]struct {
		src  Source
		id   string
		want string
	}{
		"verified":       {buildServer(t, priv, pkg+id+"  build-0.5.0-deb\n", false), id, BuildVerified},
		"another build":  {buildServer(t, priv, pkg+id+"  build-0.5.0-deb\n", false), strings.Repeat("ef", 32), BuildModified},
		"rpm not listed": {buildServer(t, priv, pkg+id+"  build-0.5.0-rpm\n", false), id, BuildUnlisted},
		"before IDs":     {buildServer(t, priv, pkg, false), id, BuildUnlisted},
		"another key":    {buildServer(t, otherPriv, pkg+id+"  build-0.5.0-deb\n", false), id, BuildModified},
		"not released":   {buildServer(t, priv, pkg, true), id, BuildUnlisted},
		"offline":        {Source{Repo: "me/whiterose", API: "http://127.0.0.1:1"}, id, BuildUnknown},
	} {
		got, err := c.src.VerifyBuild(ctx, "0.5.0", "deb", c.id, pub)
		if got != c.want || (got != BuildVerified) != (err != nil) {
			t.Errorf("%s: %s (%v), want %s", name, got, err, c.want)
		}
	}
}
