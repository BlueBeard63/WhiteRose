package store

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"regexp"
	"strings"

	"whiterose/daemon/internal/config"
)

// Key describes a known setting: its default and what values it accepts.
type Key struct {
	Default json.RawMessage
	Check   func(json.RawMessage) error
	// ReadOnly keys are changed through their own requests (calendar
	// sources, whose links are secrets), not with a plain set.
	ReadOnly bool
}

func into[T any](check func(T) error) func(json.RawMessage) error {
	return func(raw json.RawMessage) error {
		dec := json.NewDecoder(bytes.NewReader(raw))
		dec.DisallowUnknownFields()
		var v T
		if err := dec.Decode(&v); err != nil {
			return err
		}
		if check != nil {
			return check(v)
		}
		return nil
	}
}

func maxLen[T any](n int) func([]T) error {
	return func(v []T) error {
		if len(v) > n {
			return fmt.Errorf("at most %d entries", n)
		}
		return nil
	}
}

// Calculation is one launcher calculator history entry.
type Calculation struct {
	Expr   string `json:"expr"`
	Result string `json:"result"`
}

// CalendarSource is a calendar as stored: a link is kept in the secret
// store, referenced by Secret; files are plain paths.
type CalendarSource struct {
	Name   string `json:"name"`
	Color  string `json:"color,omitempty"`
	Path   string `json:"path,omitempty"`
	Secret string `json:"secret,omitempty"` // id of the link in the secret store
}

var colorRe = regexp.MustCompile(`^#[0-9A-Fa-f]{6}$`)

// Known settings. Dock pins default to the Figma dock's apps.
var Known = map[string]Key{
	"dock.pinned": {
		Default: json.RawMessage(`["org.gnome.Nautilus","firefox_firefox","org.gnome.Ptyxis","code_code","spotify","discord_discord"]`),
		Check:   into(maxLen[string](64)),
	},
	"launcher.launches": {
		Default: json.RawMessage(`{}`),
		Check: into(func(m map[string]int) error {
			if len(m) > 2000 {
				return errors.New("too many apps")
			}
			return nil
		}),
	},
	"launcher.commands":     {Default: json.RawMessage(`[]`), Check: into(maxLen[string](50))},
	"launcher.calculations": {Default: json.RawMessage(`[]`), Check: into(maxLen[Calculation](20))},
	"quick.doNotDisturb":    {Default: json.RawMessage(`false`), Check: into[bool](nil)},
	// Unix ms when Night Light turns itself off; 0 = off.
	"quick.nightLight.until": {Default: json.RawMessage(`0`), Check: into(func(v float64) error {
		if v < 0 {
			return errors.New("must be 0 or a time")
		}
		return nil
	})},
	"quick.nightLight.temperature": {Default: json.RawMessage(`4000`), Check: into(func(v int) error {
		if v < 1900 || v > 6500 {
			return errors.New("must be 1900-6500 K")
		}
		return nil
	})},
	"weather.city":      {Default: json.RawMessage(`""`), Check: into(func(s string) error { return limit(s, 200) })},
	"weather.latitude":  {Default: json.RawMessage(`null`), Check: into(func(v *float64) error { return rangeCheck(v, -90, 90) })},
	"weather.longitude": {Default: json.RawMessage(`null`), Check: into(func(v *float64) error { return rangeCheck(v, -180, 180) })},
	"weather.units": {Default: json.RawMessage(`"celsius"`), Check: into(func(s string) error {
		if s != "celsius" && s != "fahrenheit" {
			return errors.New(`must be "celsius" or "fahrenheit"`)
		}
		return nil
	})},
	"calendar.sources": {
		Default:  json.RawMessage(`[]`),
		ReadOnly: true,
		Check: into(func(v []CalendarSource) error {
			if len(v) > 50 {
				return errors.New("at most 50 calendars")
			}
			for _, c := range v {
				if c.Name == "" || c.Path == "" && c.Secret == "" {
					return errors.New("each calendar needs a name and a link or file")
				}
				if c.Color != "" && !colorRe.MatchString(c.Color) {
					return fmt.Errorf("colour %q must look like #E8A3AE", c.Color)
				}
			}
			return nil
		}),
	},
	// A display layout being tried out: the compositor shows it until
	// `until` (unix ms), then goes back to "displays" unless Settings
	// confirms it first. null when nothing is being tried.
	"displays.test": {
		Default: json.RawMessage(`null`),
		Check: func(raw json.RawMessage) error {
			if string(bytes.TrimSpace(raw)) == "null" {
				return nil
			}
			var t struct {
				Config json.RawMessage `json:"config"`
				Until  float64         `json:"until"`
			}
			dec := json.NewDecoder(bytes.NewReader(raw))
			dec.DisallowUnknownFields()
			if err := dec.Decode(&t); err != nil {
				return err
			}
			if t.Until <= 0 {
				return errors.New("until must be a time")
			}
			return checkDisplays(t.Config)
		},
	},
	"bar.batteryPercent": {Default: json.RawMessage(`true`), Check: into[bool](nil)},
	// What the top bar shows.
	"bar.workspaces":     {Default: json.RawMessage(`true`), Check: into[bool](nil)},
	"bar.workspaceStyle": {Default: json.RawMessage(`"dots"`), Check: into(oneOf("dots", "numbers"))},
	"bar.media":          {Default: json.RawMessage(`true`), Check: into[bool](nil)},
	"bar.network":        {Default: json.RawMessage(`true`), Check: into[bool](nil)},
	"bar.bluetooth":      {Default: json.RawMessage(`true`), Check: into[bool](nil)},
	"bar.volume":         {Default: json.RawMessage(`true`), Check: into[bool](nil)},
	"bar.bell":           {Default: json.RawMessage(`true`), Check: into[bool](nil)},
	"bar.date":           {Default: json.RawMessage(`true`), Check: into[bool](nil)},
	"bar.timeFormat":     {Default: json.RawMessage(`"24h"`), Check: into(oneOf("24h", "12h"))},
	"bar.seconds":        {Default: json.RawMessage(`false`), Check: into[bool](nil)},
	// A bar on every screen, or only the main one (built-in first).
	"bar.screens": {Default: json.RawMessage(`"all"`), Check: into(oneOf("all", "main"))},
	// How long notification popups stay (seconds); critical ones always stay.
	"notifications.popupSeconds": {Default: json.RawMessage(`5`), Check: into(func(v int) error {
		if v < 3 || v > 30 {
			return errors.New("must be 3-30 seconds")
		}
		return nil
	})},
	// Apps whose notifications are kept quietly in the panel (no popup).
	"notifications.muted": {Default: json.RawMessage(`[]`), Check: into(maxLen[string](200))},
	// Apps that have sent notifications (recorded by the shell, for the
	// Notifications page's list).
	"notifications.apps": {Default: json.RawMessage(`[]`), Check: into(maxLen[string](200))},
	// Settings pages switched off on the Modules page (core pages can't be).
	"settings.modules.disabled": {Default: json.RawMessage(`[]`), Check: into(maxLen[string](200))},
	"displays": {
		Default: json.RawMessage(`{"monitor":[],"profile":[]}`),
		Check:   checkDisplays,
	},
	// Appearance: dark, light, or auto (light between sunrise and sunset).
	"appearance.theme": {Default: json.RawMessage(`"dark"`), Check: into(oneOf("dark", "light", "auto"))},
	"appearance.accent": {Default: json.RawMessage(`"#E8A3AE"`), Check: into(func(s string) error {
		if !hexColor.MatchString(s) {
			return errors.New("must be a colour like #E8A3AE")
		}
		return nil
	})},
	"appearance.font": {Default: json.RawMessage(`"Geist"`), Check: into(func(s string) error {
		if strings.TrimSpace(s) == "" {
			return errors.New("needs a font name")
		}
		return limit(s, 100)
	})},
	"appearance.size": {Default: json.RawMessage(`"default"`), Check: into(oneOf("compact", "default", "large"))},
	// The wallpaper: "builtin:<name>" or an image's path.
	"wallpaper.current": {Default: json.RawMessage(`"builtin:rose"`), Check: into(func(s string) error {
		if strings.TrimSpace(s) == "" {
			return errors.New("needs a wallpaper")
		}
		return limit(s, 4096)
	})},
	// Folders whose images are offered (and rotated through), and single images added.
	"wallpaper.folders": {Default: json.RawMessage(`["~/Pictures/Wallpapers"]`), Check: into(paths(20))},
	"wallpaper.files":   {Default: json.RawMessage(`[]`), Check: into(paths(200))},
	// Change to the next image every this many minutes (0: never).
	"wallpaper.rotateMinutes": {Default: json.RawMessage(`0`), Check: into(func(v int) error {
		switch v {
		case 0, 15, 30, 60, 1440:
			return nil
		}
		return errors.New("must be 0, 15, 30, 60 or 1440 minutes")
	})},
	// The first-run app setup has been offered and run (or skipped).
	"apps.setupDone": {Default: json.RawMessage(`false`), Check: into[bool](nil)},
	// The terminal and code editor "Open in…" uses: a command name ("" picks
	// the first installed one).
	"apps.terminal": {Default: json.RawMessage(`""`), Check: into(commandName)},
	"apps.editor":   {Default: json.RawMessage(`""`), Check: into(commandName)},
	// WhiteRose Files' shortcuts changed from their defaults: {"new-tab": "Ctrl+T", …}.
	"files.keys": {Default: json.RawMessage(`{}`), Check: into(appKeys)},
	// WhiteRose Files (Settings › Files): where it starts, how items open,
	// what shows, folders without a view of their own, lists, the search index.
	"files.startIn":      {Default: json.RawMessage(`"last"`), Check: into(oneOf("last", "home", "folder"))},
	"files.startFolder":  {Default: json.RawMessage(`""`), Check: into(pathOrEmpty)},
	"files.openWith":     {Default: json.RawMessage(`"double"`), Check: into(oneOf("double", "single"))},
	"files.confirmTrash": {Default: json.RawMessage(`false`), Check: into[bool](nil)},
	"files.hidden":       {Default: json.RawMessage(`false`), Check: into[bool](nil)},
	"files.details":      {Default: json.RawMessage(`false`), Check: into[bool](nil)},
	"files.adaptive":     {Default: json.RawMessage(`true`), Check: into[bool](nil)},
	"files.zoom": {Default: json.RawMessage(`1`), Check: into(func(v float64) error {
		if v < 0.75 || v > 1.6 {
			return errors.New("must be 0.75 to 1.6")
		}
		return nil
	})},
	// Tags (Files): their names and colours, in the order shown. The tags
	// themselves are on the files (user.xdg.tags).
	"files.tags":        {Default: json.RawMessage(`[{"name":"Important","colour":"red"},{"name":"Work","colour":"blue"},{"name":"Personal","colour":"green"}]`), Check: into(tagList)},
	"files.foldersAt":   {Default: json.RawMessage(`"first"`), Check: into(oneOf("first", "mixed", "last"))},
	"files.columns":     {Default: json.RawMessage(`["size","modified","kind"]`), Check: into(listColumns)},
	"files.widths":      {Default: json.RawMessage(`{}`), Check: into(columnWidths)},
	"files.folderSizes": {Default: json.RawMessage(`true`), Check: into[bool](nil)},
	"files.defaultView": {Default: json.RawMessage(`{"mode":"grid","sortBy":"name","descending":false,"group":""}`), Check: into(func(v FilesView) error {
		if err := oneOf("grid", "list", "columns", "gallery", "cards", "usage")(v.Mode); err != nil {
			return err
		}
		if err := oneOf("name", "size", "modified", "created", "kind")(v.SortBy); err != nil {
			return err
		}
		return oneOf("", "kind", "modified", "size", "name", "tag")(v.Group)
	})},
	// The search index: drives too (not just home), folders left out, and
	// when it was last asked to be made again (unix ms; Files does it).
	"files.indexDrives":  {Default: json.RawMessage(`true`), Check: into[bool](nil)},
	"files.indexExclude": {Default: json.RawMessage(`[]`), Check: into(paths(100))},
	"files.reindex":      {Default: json.RawMessage(`0`), Check: into[float64](nil)},
	// Privacy: lock after this many idle minutes (0: never), screens off
	// after this many (0: never), lock before sleeping, and what's remembered.
	"privacy.lockAfter":        {Default: json.RawMessage(`5`), Check: into(minutes(0, 1, 2, 5, 10, 15, 30, 60))},
	"privacy.screenOffAfter":   {Default: json.RawMessage(`10`), Check: into(minutes(0, 1, 2, 5, 10, 15, 30, 60))},
	"privacy.lockOnSuspend":    {Default: json.RawMessage(`true`), Check: into[bool](nil)},
	"privacy.clipboardHistory": {Default: json.RawMessage(`true`), Check: into[bool](nil)},
	"privacy.recentFiles":      {Default: json.RawMessage(`true`), Check: into[bool](nil)},
	// Keyboard layouts, first is the default: UK English to start with.
	// An empty list means the system's (/etc/default/keyboard or the X11
	// keyboard config).
	"keyboard.layouts": {Default: json.RawMessage(`[{"layout":"gb"}]`), Check: into(func(v []KeyboardLayout) error {
		if len(v) > 8 {
			return errors.New("at most 8 layouts")
		}
		for _, l := range v {
			if !xkbName.MatchString(l.Layout) || (l.Variant != "" && !xkbName.MatchString(l.Variant)) {
				return fmt.Errorf("invalid layout %q (%q)", l.Layout, l.Variant)
			}
		}
		return nil
	})},
	"keyboard.repeatDelay": {Default: json.RawMessage(`300`), Check: into(func(v int) error {
		if v < 150 || v > 1000 {
			return errors.New("must be 150-1000 ms")
		}
		return nil
	})},
	"keyboard.repeatRate": {Default: json.RawMessage(`30`), Check: into(func(v int) error {
		if v < 5 || v > 80 {
			return errors.New("must be 5-80 per second")
		}
		return nil
	})},
	// Shortcuts changed from their defaults: action id -> key combos
	// ("SUPER+SHIFT+S"); an empty list turns the shortcut off.
	"shortcuts.keys": {Default: json.RawMessage(`{}`), Check: into(func(v map[string][]string) error {
		if len(v) > 200 {
			return errors.New("too many shortcuts")
		}
		for id, combos := range v {
			if !shortcutID.MatchString(id) {
				return fmt.Errorf("invalid shortcut id %q", id)
			}
			if err := checkCombos(combos); err != nil {
				return fmt.Errorf("%s: %w", id, err)
			}
		}
		return nil
	})},
	// Shortcuts you added: run a command.
	"shortcuts.custom": {Default: json.RawMessage(`[]`), Check: into(func(v []CustomShortcut) error {
		if len(v) > 100 {
			return errors.New("at most 100 custom shortcuts")
		}
		seen := map[string]bool{}
		for _, c := range v {
			switch {
			case !shortcutID.MatchString(c.ID) || !strings.HasPrefix(c.ID, "custom-"):
				return fmt.Errorf("invalid id %q (want custom-...)", c.ID)
			case seen[c.ID]:
				return fmt.Errorf("id %q used twice", c.ID)
			case strings.TrimSpace(c.Name) == "" || len(c.Name) > 100:
				return fmt.Errorf("%s: name must be 1-100 characters", c.ID)
			case strings.TrimSpace(c.Command) == "" || len(c.Command) > 2000:
				return fmt.Errorf("%s: command must be 1-2000 characters", c.ID)
			}
			seen[c.ID] = true
			if err := checkCombos(c.Keys); err != nil {
				return fmt.Errorf("%s: %w", c.ID, err)
			}
		}
		return nil
	})},
}

// KeyboardLayout is an XKB layout and optional variant ("gb", "extd").
type KeyboardLayout struct {
	Layout  string `json:"layout"`
	Variant string `json:"variant,omitempty"`
}

// CustomShortcut runs Command when one of Keys is pressed.
type CustomShortcut struct {
	ID      string   `json:"id"`
	Name    string   `json:"name"`
	Command string   `json:"command"`
	Keys    []string `json:"keys"`
}

// A folder view: grid, sorted by name, and so on (files.defaultView).
type FilesView struct {
	Mode       string `json:"mode"`
	SortBy     string `json:"sortBy"`
	Descending bool   `json:"descending"`
	Group      string `json:"group"`
}

// A tag in files.tags.
type FilesTag struct {
	Name   string `json:"name"`
	Colour string `json:"colour"`
}

func tagList(tags []FilesTag) error {
	if len(tags) > 100 {
		return errors.New("at most 100 tags")
	}
	seen := map[string]bool{}
	for _, t := range tags {
		n := strings.TrimSpace(t.Name)
		if n == "" || n != t.Name || len(n) > 60 || strings.ContainsAny(n, ",\n\t") {
			return fmt.Errorf("%q: not a tag name", t.Name)
		}
		if seen[strings.ToLower(n)] {
			return fmt.Errorf("%q twice", n)
		}
		seen[strings.ToLower(n)] = true
		if err := oneOf("rose", "red", "orange", "yellow", "green", "teal", "blue", "purple", "pink", "grey")(t.Colour); err != nil {
			return err
		}
	}
	return nil
}

func listColumns(cols []string) error {
	seen := map[string]bool{}
	for _, c := range cols {
		if err := oneOf("size", "modified", "created", "kind", "tags")(c); err != nil {
			return err
		}
		if seen[c] {
			return fmt.Errorf("%s twice", c)
		}
		seen[c] = true
	}
	return nil
}

func columnWidths(m map[string]float64) error {
	for k, w := range m {
		if err := oneOf("size", "modified", "created", "kind")(k); err != nil {
			return err
		}
		if w < 60 || w > 600 {
			return fmt.Errorf("%s: width must be 60 to 600", k)
		}
	}
	return nil
}

func pathOrEmpty(s string) error {
	if s == "" || strings.HasPrefix(s, "/") || strings.HasPrefix(s, "~/") {
		if len(s) > 4096 {
			return errors.New("too long")
		}
		return nil
	}
	return errors.New("must be an absolute path")
}

// An app's own shortcuts: short ids to Qt key sequences ("Ctrl+Shift+T", "F5").
func appKeys(m map[string]string) error {
	if len(m) > 100 {
		return errors.New("too many")
	}
	for id, k := range m {
		if len(id) == 0 || len(id) > 40 || len(k) == 0 || len(k) > 40 {
			return fmt.Errorf("%q: bad shortcut", id)
		}
		for _, c := range id + k {
			if !(c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z' || c >= '0' && c <= '9' || c == '-' || c == '+') {
				return fmt.Errorf("%q: bad shortcut", id)
			}
		}
	}
	return nil
}

// A program's name on $PATH (no path, no arguments), or "".
func commandName(s string) error {
	if len(s) > 64 {
		return errors.New("too long")
	}
	for _, c := range s {
		if !(c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z' || c >= '0' && c <= '9' || c == '-' || c == '_' || c == '.') {
			return errors.New("must be a program's name")
		}
	}
	if strings.HasPrefix(s, ".") || strings.HasPrefix(s, "-") {
		return errors.New("must be a program's name")
	}
	return nil
}

func oneOf(allowed ...string) func(string) error {
	return func(s string) error {
		for _, a := range allowed {
			if s == a {
				return nil
			}
		}
		return fmt.Errorf("must be one of %s", strings.Join(allowed, ", "))
	}
}

func minutes(allowed ...int) func(int) error {
	return func(v int) error {
		for _, a := range allowed {
			if v == a {
				return nil
			}
		}
		return fmt.Errorf("must be one of %v minutes", allowed)
	}
}

func paths(n int) func([]string) error {
	return func(v []string) error {
		if len(v) > n {
			return fmt.Errorf("at most %d entries", n)
		}
		for _, p := range v {
			if p == "" || len(p) > 4096 || !(strings.HasPrefix(p, "/") || strings.HasPrefix(p, "~/")) {
				return fmt.Errorf("%q must be an absolute path (or start with ~/)", p)
			}
		}
		return nil
	}
}

var (
	hexColor   = regexp.MustCompile(`^#[0-9A-Fa-f]{6}$`)
	xkbName    = regexp.MustCompile(`^[A-Za-z0-9_()-]{1,40}$`)
	shortcutID = regexp.MustCompile(`^[a-z0-9-]{1,64}$`)
	// "SUPER+SHIFT+S", "PRTSC", "SUPER+1-9": modifiers and one key. The
	// compositor checks key names against XKB.
	comboRe = regexp.MustCompile(`^([A-Za-z]+\+){0,4}[A-Za-z0-9_-]{1,40}$`)
)

func checkCombos(combos []string) error {
	if len(combos) > 4 {
		return errors.New("at most 4 key combinations")
	}
	for _, c := range combos {
		if !comboRe.MatchString(c) {
			return fmt.Errorf("invalid key combination %q", c)
		}
	}
	return nil
}

// checkDisplays validates a display layout like the daemon's config does.
func checkDisplays(raw json.RawMessage) error {
	var d config.Displays
	dec := json.NewDecoder(bytes.NewReader(raw))
	dec.DisallowUnknownFields()
	if err := dec.Decode(&d); err != nil {
		return err
	}
	return d.Validate()
}

func limit(s string, n int) error {
	if len(s) > n {
		return fmt.Errorf("at most %d characters", n)
	}
	return nil
}

func rangeCheck(v *float64, lo, hi float64) error {
	if v != nil && (*v < lo || *v > hi) {
		return fmt.Errorf("must be between %g and %g", lo, hi)
	}
	return nil
}

var keyRe = regexp.MustCompile(`^[a-z][a-zA-Z0-9_]*(\.[a-zA-Z0-9_-]+)*$`)

func checkKey(key string) error {
	if len(key) > 128 || !keyRe.MatchString(key) {
		return fmt.Errorf("bad setting name %q", key)
	}
	if _, ok := Known[key]; ok || strings.HasPrefix(key, "module.") {
		return nil
	}
	return fmt.Errorf("unknown setting %q (settings modules use module.<name>.…)", key)
}

// Validate checks a value for key before it's stored.
func Validate(key string, value json.RawMessage) error {
	if err := checkKey(key); err != nil {
		return err
	}
	if len(value) > MaxValue {
		return fmt.Errorf("%s: value too large", key)
	}
	if !json.Valid(value) {
		return fmt.Errorf("%s: not valid JSON", key)
	}
	if k, ok := Known[key]; ok && k.Check != nil {
		if err := k.Check(value); err != nil {
			return fmt.Errorf("%s: %w", key, err)
		}
	}
	return nil
}
