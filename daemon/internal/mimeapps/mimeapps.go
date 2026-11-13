// Package mimeapps is which app opens which kind of file in a WhiteRose
// session (Settings › Apps › Default apps), by the freedesktop rules
// (the "Association between MIME types and applications" spec):
//
//   - The apps are the desktop entries in $XDG_DATA_HOME/applications
//     and each $XDG_DATA_DIRS/applications (Flatpak's and Snap's too),
//     the first of an id winning; what each opens is its MimeType=.
//   - A type's default is the first installed app named for it under
//     [Default Applications] in, in order: ~/.config/whiterose-mimeapps.list,
//     ~/.config/mimeapps.list, each $XDG_CONFIG_DIRS (whiterose-, then
//     plain), then the same names (and the legacy defaults.list) in the
//     applications folders. Failing all, what GIO picks: the first
//     installed app for it in the applications folders' mimeinfo.cache
//     (so the page shows what `gio open` really opens).
//   - Yours are saved in ~/.config/whiterose-mimeapps.list: read only in a
//     WhiteRose session, so GNOME's defaults are never changed from here.
package mimeapps

import (
	"bufio"
	"os"
	"path/filepath"
	"slices"
	"sort"
	"strings"
)

// App is an installed app that opens files.
type App struct {
	ID    string   `json:"id"` // its desktop file id (org.gnome.Papers.desktop)
	Name  string   `json:"name"`
	Icon  string   `json:"icon"`
	Types []string `json:"-"`
}

// Dirs is where it all is (tests point them elsewhere).
type Dirs struct {
	ConfigHome string   // ~/.config
	ConfigDirs []string // /etc/xdg
	DataHome   string   // ~/.local/share
	DataDirs   []string // /usr/local/share, /usr/share, flatpak, snap
	Desktop    string   // "whiterose": the desktop-specific lists' prefix
}

// FromEnv: the XDG folders as the session has them.
func FromEnv() Dirs {
	home, _ := os.UserHomeDir()
	get := func(k, def string) string {
		if v := os.Getenv(k); v != "" {
			return v
		}
		return def
	}
	split := func(v string) []string {
		var out []string
		for _, p := range strings.Split(v, ":") {
			if filepath.IsAbs(p) {
				out = append(out, p)
			}
		}
		return out
	}
	return Dirs{
		ConfigHome: get("XDG_CONFIG_HOME", filepath.Join(home, ".config")),
		ConfigDirs: split(get("XDG_CONFIG_DIRS", "/etc/xdg")),
		DataHome:   get("XDG_DATA_HOME", filepath.Join(home, ".local/share")),
		DataDirs:   split(get("XDG_DATA_DIRS", "/usr/local/share:/usr/share")),
		Desktop:    "whiterose",
	}
}

// UserFile is where your defaults are saved.
func (d Dirs) UserFile() string {
	return filepath.Join(d.ConfigHome, d.Desktop+"-mimeapps.list")
}

// lists: the mimeapps lists, in the order they count.
func (d Dirs) lists() []string {
	var out []string
	add := func(dir string, legacy bool) {
		out = append(out, filepath.Join(dir, d.Desktop+"-mimeapps.list"), filepath.Join(dir, "mimeapps.list"))
		if legacy {
			out = append(out, filepath.Join(dir, "defaults.list"))
		}
	}
	add(d.ConfigHome, false)
	for _, c := range d.ConfigDirs {
		add(c, false)
	}
	add(filepath.Join(d.DataHome, "applications"), true)
	for _, c := range d.DataDirs {
		add(filepath.Join(c, "applications"), true)
	}
	return out
}

// Apps: every installed app that opens something, by id.
func (d Dirs) Apps() map[string]App {
	out := map[string]App{}
	for _, base := range append([]string{d.DataHome}, d.DataDirs...) {
		root := filepath.Join(base, "applications")
		_ = filepath.WalkDir(root, func(p string, e os.DirEntry, err error) error {
			if err != nil || e.IsDir() || !strings.HasSuffix(p, ".desktop") {
				return nil
			}
			rel, _ := filepath.Rel(root, p)
			id := strings.ReplaceAll(rel, "/", "-")
			if _, seen := out[id]; seen {
				return nil
			}
			if a, ok := readEntry(p); ok {
				a.ID = id
				out[id] = a
			} else {
				// (Hidden or not an app: an earlier one of the id is gone.)
				out[id] = App{ID: id}
			}
			return nil
		})
	}
	for id, a := range out {
		if a.Name == "" {
			delete(out, id)
		}
	}
	return out
}

func readEntry(p string) (App, bool) {
	f, err := os.Open(p)
	if err != nil {
		return App{}, false
	}
	defer f.Close()
	var a App
	in := false
	hidden := false
	sc := bufio.NewScanner(f)
	for sc.Scan() {
		l := strings.TrimSpace(sc.Text())
		if strings.HasPrefix(l, "[") {
			in = l == "[Desktop Entry]"
			continue
		}
		k, v, ok := strings.Cut(l, "=")
		if !in || !ok {
			continue
		}
		switch strings.TrimSpace(k) {
		case "Name":
			a.Name = strings.TrimSpace(v)
		case "Icon":
			a.Icon = strings.TrimSpace(v)
		case "MimeType":
			for _, m := range strings.Split(v, ";") {
				if m = strings.TrimSpace(m); m != "" {
					a.Types = append(a.Types, m)
				}
			}
		case "Hidden":
			hidden = hidden || strings.TrimSpace(v) == "true"
		case "Type":
			hidden = hidden || strings.TrimSpace(v) != "Application"
		}
	}
	if hidden || a.Name == "" {
		return App{}, false
	}
	return a, true
}

// readDefaults: a list's [Default Applications] (type → ids, in order).
func readDefaults(p string) map[string][]string {
	return readSection(p, "[Default Applications]")
}

// readSection: a list's `section` (type → ids, in order).
func readSection(p, section string) map[string][]string {
	out := map[string][]string{}
	f, err := os.Open(p)
	if err != nil {
		return out
	}
	defer f.Close()
	in := false
	sc := bufio.NewScanner(f)
	for sc.Scan() {
		l := strings.TrimSpace(sc.Text())
		if strings.HasPrefix(l, "[") {
			in = l == section
			continue
		}
		k, v, ok := strings.Cut(l, "=")
		if !in || !ok {
			continue
		}
		for _, id := range strings.Split(v, ";") {
			if id = strings.TrimSpace(id); id != "" {
				out[strings.TrimSpace(k)] = append(out[strings.TrimSpace(k)], id)
			}
		}
	}
	return out
}

// State is everything read once (the apps and the lists).
type State struct {
	Dirs  Dirs
	Apps  map[string]App
	lists []map[string][]string
	user  map[string][]string
	cache []map[string][]string // mimeinfo.cache, per applications folder
}

func Load(d Dirs) *State {
	s := &State{Dirs: d, Apps: d.Apps()}
	for _, l := range d.lists() {
		s.lists = append(s.lists, readDefaults(l))
	}
	s.user = s.lists[0]
	for _, base := range append([]string{d.DataHome}, d.DataDirs...) {
		s.cache = append(s.cache, readSection(filepath.Join(base, "applications", "mimeinfo.cache"), "[MIME Cache]"))
	}
	return s
}

// Default: the app that opens `mime` ("" if none does).
func (s *State) Default(mime string) string {
	for _, l := range s.lists {
		for _, id := range l[mime] {
			if _, ok := s.Apps[id]; ok {
				return id
			}
		}
	}
	for _, c := range s.cache {
		for _, id := range c[mime] {
			if _, ok := s.Apps[id]; ok {
				return id
			}
		}
	}
	if c := s.Capable(mime); len(c) > 0 {
		return c[0].ID
	}
	return ""
}

// Changed: whether you've set `mime` yourself (in your WhiteRose list).
func (s *State) Changed(mime string) bool { return len(s.user[mime]) > 0 }

// Capable: the apps that say they open `mime` (or a type of its family,
// for "image/*"), by name.
func (s *State) Capable(mime string) []App {
	var out []App
	for _, a := range s.Apps {
		for _, t := range a.Types {
			if t == mime || (strings.HasSuffix(mime, "/*") && strings.HasPrefix(t, strings.TrimSuffix(mime, "*"))) {
				out = append(out, a)
				break
			}
		}
	}
	sort.Slice(out, func(i, j int) bool {
		if strings.EqualFold(out[i].Name, out[j].Name) {
			return out[i].ID < out[j].ID
		}
		return strings.ToLower(out[i].Name) < strings.ToLower(out[j].Name)
	})
	return out
}

// ChangedTypes: the types you've set yourself, sorted.
func (s *State) ChangedTypes() []string {
	out := make([]string, 0, len(s.user))
	for m := range s.user {
		out = append(out, m)
	}
	sort.Strings(out)
	return out
}

// Set makes `id` open each of `mimes` (in your WhiteRose list).
func (s *State) Set(mimes []string, id string) error {
	return s.edit(func(m map[string]string) {
		for _, t := range mimes {
			m[t] = id + ";"
		}
	})
}

// Reset takes your choice for `mimes` away (WhiteRose's and the system's
// defaults count again).
func (s *State) Reset(mimes []string) error {
	return s.edit(func(m map[string]string) {
		for _, t := range mimes {
			delete(m, t)
		}
	})
}

// edit rewrites your list's [Default Applications], leaving everything
// else in it (comments, Added Associations...) as it was.
func (s *State) edit(change func(map[string]string)) error {
	p := s.Dirs.UserFile()
	text, _ := os.ReadFile(p)
	// The file as sections in order: [header, lines...]; "" holds what's
	// above the first header.
	type section struct {
		header string
		lines  []string
	}
	secs := []section{{header: ""}}
	for _, l := range strings.Split(strings.TrimRight(string(text), "\n"), "\n") {
		if t := strings.TrimSpace(l); strings.HasPrefix(t, "[") {
			secs = append(secs, section{header: t})
			continue
		}
		if len(text) > 0 {
			secs[len(secs)-1].lines = append(secs[len(secs)-1].lines, l)
		}
	}
	at := slices.IndexFunc(secs, func(x section) bool { return x.header == "[Default Applications]" })
	if at < 0 {
		secs = append(secs, section{header: "[Default Applications]"})
		at = len(secs) - 1
	}
	defaults := map[string]string{}
	for _, l := range secs[at].lines {
		if k, v, ok := strings.Cut(strings.TrimSpace(l), "="); ok {
			defaults[strings.TrimSpace(k)] = strings.TrimSpace(v)
		}
	}
	change(defaults)
	keys := make([]string, 0, len(defaults))
	for k := range defaults {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	secs[at].lines = nil
	for _, k := range keys {
		secs[at].lines = append(secs[at].lines, k+"="+defaults[k])
	}
	var b strings.Builder
	for _, sc := range secs {
		if sc.header != "" {
			b.WriteString(sc.header + "\n")
		}
		for _, l := range sc.lines {
			b.WriteString(l + "\n")
		}
	}
	if err := os.MkdirAll(filepath.Dir(p), 0o755); err != nil {
		return err
	}
	tmp := p + ".tmp"
	if err := os.WriteFile(tmp, []byte(b.String()), 0o644); err != nil {
		return err
	}
	if err := os.Rename(tmp, p); err != nil {
		return err
	}
	// Read again: what counts now.
	s.lists[0] = readDefaults(p)
	s.user = s.lists[0]
	return nil
}

// ---- categories ---------------------------------------------------------------------------

// Category is one row of the Default apps page: one choice for a family
// of types. `Show` is the type whose default the row shows.
type Category struct {
	Key   string
	Name  string
	Show  string
	Types []string
}

var Categories = []Category{
	{"browser", "Web browser", "x-scheme-handler/https", []string{"x-scheme-handler/http", "x-scheme-handler/https", "text/html", "application/xhtml+xml"}},
	{"mail", "Email", "x-scheme-handler/mailto", []string{"x-scheme-handler/mailto", "message/rfc822"}},
	{"calendar", "Calendar", "text/calendar", []string{"text/calendar"}},
	{"photos", "Photos", "image/jpeg", []string{"image/jpeg", "image/png", "image/gif", "image/webp", "image/heic", "image/avif", "image/tiff", "image/bmp", "image/svg+xml"}},
	{"music", "Music", "audio/mpeg", []string{"audio/mpeg", "audio/flac", "audio/ogg", "audio/x-wav", "audio/mp4", "audio/aac", "audio/opus", "audio/x-vorbis+ogg"}},
	{"videos", "Videos", "video/mp4", []string{"video/mp4", "video/x-matroska", "video/webm", "video/quicktime", "video/x-msvideo", "video/mpeg", "video/ogg"}},
	{"pdf", "PDFs", "application/pdf", []string{"application/pdf"}},
	{"text", "Text", "text/plain", []string{"text/plain", "text/markdown", "application/json", "application/x-yaml", "text/x-log"}},
	{"documents", "Documents", "application/vnd.openxmlformats-officedocument.wordprocessingml.document", []string{
		"application/vnd.oasis.opendocument.text", "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
		"application/msword", "application/rtf", "text/rtf"}},
	{"spreadsheets", "Spreadsheets", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", []string{
		"application/vnd.oasis.opendocument.spreadsheet", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
		"application/vnd.ms-excel", "text/csv"}},
	{"presentations", "Presentations", "application/vnd.openxmlformats-officedocument.presentationml.presentation", []string{
		"application/vnd.oasis.opendocument.presentation", "application/vnd.openxmlformats-officedocument.presentationml.presentation",
		"application/vnd.ms-powerpoint"}},
	{"archives", "Archives", "application/zip", []string{"application/zip", "application/x-tar", "application/x-compressed-tar",
		"application/x-7z-compressed", "application/x-rar", "application/vnd.rar", "application/gzip", "application/x-xz-compressed-tar"}},
	{"folders", "Folders", "inode/directory", []string{"inode/directory"}},
}

func CategoryByKey(key string) (Category, bool) {
	i := slices.IndexFunc(Categories, func(c Category) bool { return c.Key == key })
	if i < 0 {
		return Category{}, false
	}
	return Categories[i], true
}

// ---- file types by extension ----------------------------------------------------------------

// Globs reads shared-mime-info's globs2 files: extension (".docx") → type.
func (d Dirs) Globs() map[string]string {
	out := map[string]string{}
	for _, base := range append([]string{d.DataHome}, d.DataDirs...) {
		f, err := os.Open(filepath.Join(base, "mime", "globs2"))
		if err != nil {
			continue
		}
		sc := bufio.NewScanner(f)
		for sc.Scan() {
			// weight:type:glob[:flags]
			parts := strings.Split(sc.Text(), ":")
			if len(parts) < 3 || strings.HasPrefix(parts[0], "#") {
				continue
			}
			g := parts[2]
			if strings.HasPrefix(g, "*.") && !strings.ContainsAny(g[2:], "*?[") {
				ext := strings.ToLower(g[1:])
				if _, seen := out[ext]; !seen {
					out[ext] = parts[1]
				}
			}
		}
		f.Close()
	}
	return out
}

// TypeMatch is a file type found by a search.
type TypeMatch struct {
	Mime string   `json:"mime"`
	Exts []string `json:"exts"`
}

// Search: types whose extension or name matches `q` (".docx", "docx",
// "epub", "markdown"), at most 20, exact extensions first.
func Search(q string, globs map[string]string, apps map[string]App) []TypeMatch {
	q = strings.ToLower(strings.TrimSpace(q))
	if q == "" {
		return nil
	}
	exts := map[string][]string{}
	for e, m := range globs {
		exts[m] = append(exts[m], e)
	}
	for _, a := range apps {
		for _, t := range a.Types {
			if _, ok := exts[t]; !ok {
				exts[t] = nil
			}
		}
	}
	ext := q
	if !strings.HasPrefix(ext, ".") {
		ext = "." + ext
	}
	var exact, other []TypeMatch
	for m, es := range exts {
		sort.Strings(es)
		tm := TypeMatch{Mime: m, Exts: es}
		switch {
		case slices.Contains(es, ext):
			exact = append(exact, tm)
		case strings.Contains(m, q) || slices.ContainsFunc(es, func(e string) bool { return strings.HasPrefix(e, ext) }):
			other = append(other, tm)
		}
	}
	sort.Slice(exact, func(i, j int) bool { return exact[i].Mime < exact[j].Mime })
	sort.Slice(other, func(i, j int) bool { return other[i].Mime < other[j].Mime })
	out := append(exact, other...)
	if len(out) > 20 {
		out = out[:20]
	}
	return out
}
