package mimeapps

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func write(t *testing.T, p, text string) {
	t.Helper()
	os.MkdirAll(filepath.Dir(p), 0o755)
	if err := os.WriteFile(p, []byte(text), 0o644); err != nil {
		t.Fatal(err)
	}
}

func entry(name, types string) string {
	return "[Desktop Entry]\nType=Application\nName=" + name + "\nMimeType=" + types + "\nExec=x %U\n"
}

// A user, a system and a session: Papers and Writer installed, Okular not.
func setup(t *testing.T) Dirs {
	t.Helper()
	r := t.TempDir()
	d := Dirs{
		ConfigHome: filepath.Join(r, "home/.config"),
		ConfigDirs: []string{filepath.Join(r, "etc/xdg")},
		DataHome:   filepath.Join(r, "home/.local/share"),
		DataDirs:   []string{filepath.Join(r, "usr/share"), filepath.Join(r, "flatpak")},
		Desktop:    "whiterose",
	}
	write(t, filepath.Join(r, "usr/share/applications/org.gnome.Papers.desktop"), entry("Papers", "application/pdf;image/tiff;"))
	write(t, filepath.Join(r, "usr/share/applications/libreoffice-writer.desktop"), entry("LibreOffice Writer", "application/vnd.oasis.opendocument.text;application/msword;"))
	write(t, filepath.Join(r, "usr/share/applications/org.gnome.Loupe.desktop"), entry("Image Viewer", "image/jpeg;image/png;"))
	write(t, filepath.Join(r, "flatpak/applications/org.gimp.GIMP.desktop"), entry("GIMP", "image/jpeg;image/png;"))
	// Hidden in the user's folder: that app is gone for this user.
	write(t, filepath.Join(r, "home/.local/share/applications/org.gimp.GIMP.desktop"), "[Desktop Entry]\nHidden=true\n")
	write(t, filepath.Join(r, "usr/share/applications/not-an-app.desktop"), "[Desktop Entry]\nType=Link\nName=x\n")
	// WhiteRose's own list: Okular (not installed) first, Writer next.
	write(t, filepath.Join(r, "etc/xdg/whiterose-mimeapps.list"), "[Default Applications]\napplication/msword=okular.desktop;libreoffice-writer.desktop;\n")
	// GNOME's would say otherwise: never read in a WhiteRose session.
	write(t, filepath.Join(r, "usr/share/applications/gnome-mimeapps.list"), "[Default Applications]\napplication/pdf=org.gnome.Evince.desktop\n")
	write(t, filepath.Join(r, "usr/share/applications/defaults.list"), "[Default Applications]\nimage/jpeg=org.gnome.Loupe.desktop\n")
	// GIO's own order, when no list names one: GIMP (gone) then Loupe.
	write(t, filepath.Join(r, "usr/share/applications/mimeinfo.cache"), "[MIME Cache]\nimage/png=org.gimp.GIMP.desktop;org.gnome.Loupe.desktop;\n")
	return d
}

func TestDefaults(t *testing.T) {
	d := setup(t)
	s := Load(d)
	if _, ok := s.Apps["org.gimp.GIMP.desktop"]; ok {
		t.Error("an app hidden by the user is listed")
	}
	if _, ok := s.Apps["not-an-app.desktop"]; ok {
		t.Error("a link counts as an app")
	}
	for mime, want := range map[string]string{
		"application/msword": "libreoffice-writer.desktop", // the first installed of WhiteRose's list
		"image/jpeg":         "org.gnome.Loupe.desktop",    // the legacy defaults.list
		"application/pdf":    "org.gnome.Papers.desktop",   // nobody says: the one that opens it
		"image/png":          "org.gnome.Loupe.desktop",    // GIO's order (mimeinfo.cache)
		"video/mp4":          "",
	} {
		if got := s.Default(mime); got != want {
			t.Errorf("%s: %q, want %q", mime, got, want)
		}
	}
	if c := s.Capable("image/*"); len(c) != 2 || c[0].Name != "Image Viewer" || c[1].Name != "Papers" {
		t.Errorf("image/*: %+v", c)
	}
}

func TestSetAndReset(t *testing.T) {
	d := setup(t)
	// Something of yours already there: kept.
	write(t, d.UserFile(), "# mine\n[Added Associations]\nimage/png=org.gimp.GIMP.desktop;\n\n[Default Applications]\ntext/plain=org.gnome.TextEditor.desktop;\n")
	s := Load(d)
	if err := s.Set([]string{"application/pdf", "image/tiff"}, "org.gnome.Papers.desktop"); err != nil {
		t.Fatal(err)
	}
	if s.Default("image/tiff") != "org.gnome.Papers.desktop" || !s.Changed("image/tiff") || s.Changed("image/jpeg") {
		t.Error("not set")
	}
	b, _ := os.ReadFile(d.UserFile())
	text := string(b)
	for _, want := range []string{"# mine", "[Added Associations]\nimage/png=org.gimp.GIMP.desktop;", "text/plain=org.gnome.TextEditor.desktop;", "application/pdf=org.gnome.Papers.desktop;"} {
		if !strings.Contains(text, want) {
			t.Errorf("no %q in\n%s", want, text)
		}
	}
	if err := s.Reset([]string{"image/tiff"}); err != nil {
		t.Fatal(err)
	}
	if s.Changed("image/tiff") || !s.Changed("application/pdf") {
		t.Error("reset")
	}
	// A list that didn't exist: made.
	d2 := setup(t)
	if err := Load(d2).Set([]string{"text/csv"}, "libreoffice-writer.desktop"); err != nil {
		t.Fatal(err)
	}
	if b, _ := os.ReadFile(d2.UserFile()); string(b) != "[Default Applications]\ntext/csv=libreoffice-writer.desktop;\n" {
		t.Errorf("%q", b)
	}
}

func TestSearchAndCategories(t *testing.T) {
	d := setup(t)
	write(t, filepath.Join(d.DataDirs[0], "mime/globs2"), "# comment\n50:application/vnd.openxmlformats-officedocument.wordprocessingml.document:*.docx\n50:application/epub+zip:*.epub\n50:text/markdown:*.md\n50:text/markdown:*.markdown\n10:text/x-readme:README*\n")
	s := Load(d)
	g := d.Globs()
	if g[".docx"] != "application/vnd.openxmlformats-officedocument.wordprocessingml.document" || g["readme"] != "" {
		t.Errorf("%v", g)
	}
	for q, want := range map[string]string{".docx": "application/vnd.openxmlformats-officedocument.wordprocessingml.document", "EPUB": "application/epub+zip", "markdown": "text/markdown", "msword": "application/msword"} {
		r := Search(q, g, s.Apps)
		if len(r) == 0 || r[0].Mime != want {
			t.Errorf("%s: %+v", q, r)
		}
	}
	if r := Search("md", g, s.Apps); len(r) == 0 || r[0].Mime != "text/markdown" || strings.Join(r[0].Exts, ",") != ".markdown,.md" {
		t.Errorf("md: %+v", r)
	}
	if Search("", g, s.Apps) != nil {
		t.Error("empty search")
	}
	seen := map[string]bool{}
	for _, c := range Categories {
		if seen[c.Key] || c.Name == "" || len(c.Types) == 0 || !strings.Contains(strings.Join(c.Types, " "), c.Show) {
			t.Errorf("category %+v", c)
		}
		seen[c.Key] = true
	}
	if _, ok := CategoryByKey("documents"); !ok {
		t.Error("no documents")
	}
}
