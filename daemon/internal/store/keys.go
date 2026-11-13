package store

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"math"
	"regexp"
	"strconv"
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
	// Unix ms when Night light turns itself off; 0 = off.
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
	"files.startIn":      {Default: json.RawMessage(`"last"`), Check: into(oneOf("last", "home", "start", "folder"))},
	"files.startFolder":  {Default: json.RawMessage(`""`), Check: into(pathOrEmpty)},
	"files.openWith":     {Default: json.RawMessage(`"double"`), Check: into(oneOf("double", "single"))},
	"files.confirmTrash": {Default: json.RawMessage(`false`), Check: into[bool](nil)},
	// Each copied file read back from the drive and checked against the original.
	"files.verifyCopies": {Default: json.RawMessage(`false`), Check: into[bool](nil)},
	// The search index, visited folders and the operations log, sealed
	// with a key from the keyring (apps/files/src/secure.rs).
	"files.encrypt":  {Default: json.RawMessage(`false`), Check: into[bool](nil)},
	"files.hidden":   {Default: json.RawMessage(`false`), Check: into[bool](nil)},
	"files.details":  {Default: json.RawMessage(`false`), Check: into[bool](nil)},
	"files.adaptive": {Default: json.RawMessage(`true`), Check: into[bool](nil)},
	"files.zoom": {Default: json.RawMessage(`1`), Check: into(func(v float64) error {
		if v < 0.75 || v > 1.6 {
			return errors.New("must be 0.75 to 1.6")
		}
		return nil
	})},
	// Tags (Files): their names and colours, in the order shown. The tags
	// themselves are on the files (user.xdg.tags).
	"files.tags": {Default: json.RawMessage(`[{"name":"Important","colour":"red"},{"name":"Work","colour":"blue"},{"name":"Personal","colour":"green"}]`), Check: into(tagList)},
	// Files' toolbar: extra buttons (ids from apps/files/qml/Actions.js), in order.
	"files.toolbar": {Default: json.RawMessage(`[]`), Check: into(idList)},
	// Files' right-click menu: built-in entries left out (ids from Actions.js).
	"files.menuHide": {Default: json.RawMessage(`[]`), Check: into(idList)},
	// Files' right-click menu: your own actions (a command run on what's chosen).
	"files.actions": {Default: json.RawMessage(`[]`), Check: into(actionList)},
	// The git branch in Files' status bar.
	"files.git": {Default: json.RawMessage(`true`), Check: into[bool](nil)},
	// Files' sidebar: shown, and the sections folded away.
	"files.sidebar":       {Default: json.RawMessage(`true`), Check: into[bool](nil)},
	"files.sidebarFolded": {Default: json.RawMessage(`[]`), Check: into(maxLen[string](10))},
	// Folders opened in the sidebar's tree.
	"files.sidebarOpen": {Default: json.RawMessage(`[]`), Check: into(maxLen[string](50))},
	"files.foldersAt":   {Default: json.RawMessage(`"first"`), Check: into(oneOf("first", "mixed", "last"))},
	"files.columns":     {Default: json.RawMessage(`["size","modified","kind"]`), Check: into(listColumns)},
	"files.widths":      {Default: json.RawMessage(`{}`), Check: into(columnWidths)},
	"files.folderSizes": {Default: json.RawMessage(`true`), Check: into[bool](nil)},
	"files.defaultView": {Default: json.RawMessage(`{"mode":"grid","sortBy":"name","descending":false,"group":""}`), Check: into(func(v FilesView) error {
		if err := oneOf("grid", "list", "columns", "cards", "usage")(v.Mode); err != nil {
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
	"privacy.lockAfter":      {Default: json.RawMessage(`5`), Check: into(minutes(0, 1, 2, 5, 10, 15, 30, 60))},
	"privacy.screenOffAfter": {Default: json.RawMessage(`10`), Check: into(minutes(0, 1, 2, 5, 10, 15, 30, 60))},
	"privacy.lockOnSuspend":  {Default: json.RawMessage(`true`), Check: into[bool](nil)},
	// How often whiterosed looks for a WhiteRose update (Settings › Updates).
	"update.check": {Default: json.RawMessage(`"daily"`), Check: into(oneOf("daily", "weekly", "manual"))},
	// App ids kept running when memory runs out (the terminal always is):
	// further back in the kill order, and never closed by the watcher.
	"memory.protected": {Default: json.RawMessage(`[]`), Check: into(maxLen[string](100))},
	// Offer pre-releases (alpha, beta) as updates and list them under Releases.
	"update.prerelease":        {Default: json.RawMessage(`false`), Check: into[bool](nil)},
	"privacy.clipboardHistory": {Default: json.RawMessage(`true`), Check: into[bool](nil)},
	"privacy.recentFiles":      {Default: json.RawMessage(`true`), Check: into[bool](nil)},
	// Mice and touchpads (Settings › Mouse & touchpad; the compositor
	// applies them through libinput). Speed is libinput's, -1 to 1.
	"mouse.naturalScroll":    {Default: json.RawMessage(`false`), Check: into[bool](nil)},
	"mouse.speed":            {Default: json.RawMessage(`0`), Check: into(pointerSpeed)},
	"mouse.acceleration":     {Default: json.RawMessage(`true`), Check: into[bool](nil)},
	"touchpad.naturalScroll": {Default: json.RawMessage(`true`), Check: into[bool](nil)},
	"touchpad.speed":         {Default: json.RawMessage(`0`), Check: into(pointerSpeed)},
	"touchpad.acceleration":  {Default: json.RawMessage(`true`), Check: into[bool](nil)},
	"touchpad.tapToClick":    {Default: json.RawMessage(`true`), Check: into[bool](nil)},
	// Shaking the pointer makes it big for a moment (the compositor).
	"mouse.shakeToFind": {Default: json.RawMessage(`true`), Check: into[bool](nil)},
	// Held, the pointer moves faster (fasterBy times); "" turns it off.
	// The modifiers still reach apps.
	"mouse.fasterWith": {Default: json.RawMessage(`"CTRL+SHIFT"`), Check: into(oneOf("", "CTRL+SHIFT", "CTRL+ALT", "SUPER+SHIFT"))},
	"mouse.fasterBy": {Default: json.RawMessage(`3`), Check: into(func(v float64) error {
		if v < 1 || v > 8 {
			return errors.New("must be 1-8")
		}
		return nil
	})},
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
	// Window layout (the compositor's layout/ module). The global default,
	// then overrides per workspace ("1"-"9") and per monitor (its ID, as in
	// "displays"); a monitor's beats a workspace's.
	"windows.layout": {Default: json.RawMessage(`"floating"`), Check: into(oneOf(layouts...))},
	"windows.layout.workspaces": {Default: json.RawMessage(`{}`), Check: into(func(m map[string]string) error {
		for ws, l := range m {
			if len(ws) != 1 || ws < "1" || ws > "9" {
				return fmt.Errorf("invalid workspace %q (want 1-9)", ws)
			}
			if err := oneOf(layouts...)(l); err != nil {
				return fmt.Errorf("workspace %s: %w", ws, err)
			}
		}
		return nil
	})},
	"windows.layout.monitors": {Default: json.RawMessage(`{}`), Check: into(func(m map[string]string) error {
		if len(m) > 32 {
			return errors.New("at most 32 monitors")
		}
		for id, l := range m {
			if !config.ValidID(id) {
				return fmt.Errorf("invalid monitor id %q", id)
			}
			if err := oneOf(layouts...)(l); err != nil {
				return fmt.Errorf("%s: %w", id, err)
			}
		}
		return nil
	})},
	// Floating windows snap to edges, corners and zones.
	"windows.snap": {Default: json.RawMessage(`true`), Check: into[bool](nil)},
	// Zone grids per monitor (its ID): [x, y, w, h] as fractions of the
	// area the bar leaves, shown while SHIFT is held during a drag. No grid:
	// two thirds and a third.
	"windows.snap.zones": {Default: json.RawMessage(`{}`), Check: into(func(m map[string][][4]float64) error {
		if len(m) > 32 {
			return errors.New("at most 32 monitors")
		}
		for id, zones := range m {
			if !config.ValidID(id) {
				return fmt.Errorf("invalid monitor id %q", id)
			}
			if err := checkZones(zones); err != nil {
				return fmt.Errorf("%s: %w", id, err)
			}
		}
		return nil
	})},
	// A workspace's own zone grid on a monitor, instead of the monitor's:
	// {"<workspace 1-9>": {"<monitor id>": [[x, y, w, h], ...]}}.
	"windows.snap.zones.workspaces": {Default: json.RawMessage(`{}`), Check: into(func(m map[string]map[string][][4]float64) error {
		for ws, grids := range m {
			if n, err := strconv.Atoi(ws); err != nil || n < 1 || n > 9 || ws != strconv.Itoa(n) {
				return fmt.Errorf("invalid workspace %q (1-9)", ws)
			}
			if len(grids) > 32 {
				return fmt.Errorf("workspace %s: at most 32 monitors", ws)
			}
			for id, zones := range grids {
				if !config.ValidID(id) {
					return fmt.Errorf("workspace %s: invalid monitor id %q", ws, id)
				}
				if err := checkZones(zones); err != nil {
					return fmt.Errorf("workspace %s, %s: %w", ws, id, err)
				}
			}
		}
		return nil
	})},
	// Gaps in pixels: between tiles, and around them. Smart: none with one tile.
	// The note (the Note module, SUPER+SHIFT+N): one text, its first line the
	// title, "[ ]" lines tasks.
	"note.text": {Default: json.RawMessage(`""`), Check: into(func(s string) error { return limit(s, 20000) })},
	// ALT+TAB: apps with windows on any workspace, or only this one.
	"windows.appSwitcher.workspaces": {Default: json.RawMessage(`"all"`), Check: into(oneOf("all", "current"))},
	"windows.gaps.inner":             {Default: json.RawMessage(`8`), Check: into(pixels(0, 64))},
	"windows.gaps.outer":             {Default: json.RawMessage(`8`), Check: into(pixels(0, 64))},
	"windows.gaps.smart":             {Default: json.RawMessage(`false`), Check: into[bool](nil)},
	// Master layout: the master tile's share of the area, and its side.
	"windows.master.ratio": {Default: json.RawMessage(`0.55`), Check: into(func(v float64) error {
		if v < 0.1 || v > 0.9 {
			return errors.New("must be 0.1-0.9")
		}
		return nil
	})},
	"windows.master.side": {Default: json.RawMessage(`"left"`), Check: into(oneOf("left", "right", "top", "bottom"))},
	// Dwindle: which way new tiles split (auto: by the tile's shape).
	"windows.dwindle.split": {Default: json.RawMessage(`"auto"`), Check: into(oneOf("auto", "horizontal", "vertical"))},
	// Window rules, in order: what to do with a window when it opens,
	// matched by app id and/or title (regular expressions). Every rule that
	// matches applies; for the same action the later rule wins.
	"windows.rules": {Default: json.RawMessage(`[]`), Check: into(checkWindowRules)},
	// Apps shown and hidden with a key, sliding down from the island.
	"windows.scratchpads": {Default: json.RawMessage(`[]`), Check: into(checkScratchpads)},
}

// The window layouts (windows.layout and its overrides).
var layouts = []string{"floating", "dwindle", "master"}

// WindowRule is one of windows.rules. Sizes and positions are pixels or
// "N%" of the monitor's usable area; Position may also be "centre".
type WindowRule struct {
	App        string            `json:"app,omitempty"`
	Title      string            `json:"title,omitempty"`
	Workspace  *int              `json:"workspace,omitempty"`
	Monitor    string            `json:"monitor,omitempty"`
	Float      *bool             `json:"float,omitempty"`
	Size       []json.RawMessage `json:"size,omitempty"`
	Position   json.RawMessage   `json:"position,omitempty"`
	Maximized  *bool             `json:"maximized,omitempty"`
	Fullscreen *bool             `json:"fullscreen,omitempty"`
	Opacity    []float64         `json:"opacity,omitempty"`
	OnTop      *bool             `json:"onTop,omitempty"`
	Zone       *int              `json:"zone,omitempty"`
}

func checkWindowRules(rules []WindowRule) error {
	if len(rules) > 200 {
		return errors.New("at most 200 rules")
	}
	for i, r := range rules {
		if err := checkWindowRule(r); err != nil {
			return fmt.Errorf("rule %d: %w", i+1, err)
		}
	}
	return nil
}

func checkWindowRule(r WindowRule) error {
	if r.App == "" && r.Title == "" {
		return errors.New("needs an app id or a title")
	}
	for _, p := range []string{r.App, r.Title} {
		if len(p) > 500 {
			return errors.New("pattern too long")
		}
		if _, err := regexp.Compile(p); err != nil {
			return err
		}
	}
	if r.Workspace != nil && (*r.Workspace < 1 || *r.Workspace > 9) {
		return errors.New("workspace must be 1-9")
	}
	if r.Zone != nil && (*r.Zone < 1 || *r.Zone > 64) {
		return errors.New("zone must be 1-64")
	}
	if r.Size != nil {
		if len(r.Size) != 2 {
			return errors.New("size is [width, height]")
		}
		for _, l := range r.Size {
			if err := checkRuleLength(l, 1); err != nil {
				return fmt.Errorf("size: %w", err)
			}
		}
	}
	if r.Position != nil {
		var s string
		var xy []json.RawMessage
		switch {
		case json.Unmarshal(r.Position, &s) == nil:
			if s != "centre" && s != "center" {
				return errors.New(`position is [x, y] or "centre"`)
			}
		case json.Unmarshal(r.Position, &xy) == nil && len(xy) == 2:
			for _, l := range xy {
				if err := checkRuleLength(l, 0); err != nil {
					return fmt.Errorf("position: %w", err)
				}
			}
		default:
			return errors.New(`position is [x, y] or "centre"`)
		}
	}
	if r.Opacity != nil {
		if len(r.Opacity) != 2 {
			return errors.New("opacity is [focused, unfocused]")
		}
		for _, o := range r.Opacity {
			if o < 0.1 || o > 1 {
				return errors.New("opacity must be 0.1-1")
			}
		}
	}
	return nil
}

// Scratchpad is one of windows.scratchpads: the key runs Command, and the
// first window that process opens is shown and hidden by the key after.
// No keys is allowed (another shortcut took its key): it's off until it
// has one.
// Size is pixels or "N%" of the monitor's usable area; Place is "top"
// (under the island) or "centre".
type Scratchpad struct {
	ID         string            `json:"id"`
	Name       string            `json:"name"`
	Command    string            `json:"command"`
	Keys       []string          `json:"keys"`
	Size       []json.RawMessage `json:"size,omitempty"`
	Place      string            `json:"place,omitempty"`
	HideOnBlur bool              `json:"hideOnBlur,omitempty"`
}

func checkScratchpads(pads []Scratchpad) error {
	if len(pads) > 50 {
		return errors.New("at most 50 scratchpads")
	}
	seen := map[string]bool{}
	for i, p := range pads {
		switch {
		case !shortcutID.MatchString(p.ID):
			return fmt.Errorf("scratchpad %d: invalid id %q", i+1, p.ID)
		case seen[p.ID]:
			return fmt.Errorf("scratchpad %d: id %q used twice", i+1, p.ID)
		}
		seen[p.ID] = true
		if err := checkScratchpad(p); err != nil {
			return fmt.Errorf("%s: %w", p.ID, err)
		}
	}
	return nil
}

func checkScratchpad(p Scratchpad) error {
	switch {
	case len(p.Name) > 100:
		return errors.New("name must be at most 100 characters")
	case strings.TrimSpace(p.Command) == "" || len(p.Command) > 2000:
		return errors.New("command must be 1-2000 characters")
	case p.Place != "" && p.Place != "top" && p.Place != "centre" && p.Place != "center":
		return errors.New(`place is "top" or "centre"`)
	}
	if err := checkCombos(p.Keys); err != nil {
		return err
	}
	if p.Size != nil {
		if len(p.Size) != 2 {
			return errors.New("size is [width, height]")
		}
		for _, l := range p.Size {
			if err := checkRuleLength(l, 1); err != nil {
				return fmt.Errorf("size: %w", err)
			}
		}
	}
	return nil
}

// checkRuleLength: whole pixels (at least min) or "N%" (0-100).
func checkRuleLength(raw json.RawMessage, min int) error {
	var px int
	if json.Unmarshal(raw, &px) == nil {
		if px < min || px > 100000 {
			return fmt.Errorf("must be at least %d px", min)
		}
		return nil
	}
	var s string
	if json.Unmarshal(raw, &s) == nil {
		if n, ok := strings.CutSuffix(strings.TrimSpace(s), "%"); ok {
			if f, err := strconv.ParseFloat(strings.TrimSpace(n), 64); err == nil && f >= 0 && f <= 100 {
				return nil
			}
		}
	}
	return errors.New(`must be pixels or "N%"`)
}

func pixels(lo, hi int) func(int) error {
	return func(v int) error {
		if v < lo || v > hi {
			return fmt.Errorf("must be %d-%d px", lo, hi)
		}
		return nil
	}
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

// An id in a list of Files' buttons or menu entries.
func idList(ids []string) error {
	if len(ids) > 64 {
		return errors.New("too many")
	}
	for _, id := range ids {
		if id == "" || len(id) > 40 || strings.ContainsAny(id, " \t\n") {
			return fmt.Errorf("%q isn't an id", id)
		}
	}
	return nil
}

// FilesAction is one of your own actions in Files' right-click menu.
type FilesAction struct {
	Name    string `json:"name"`
	Command string `json:"command"`
	// files | folders | any
	On string `json:"on"`
	// Extensions it's for ("jpg,png"), "" for any.
	Types    string `json:"types"`
	Terminal bool   `json:"terminal"`
}

func actionList(list []FilesAction) error {
	if len(list) > 50 {
		return errors.New("at most 50 actions")
	}
	for _, a := range list {
		if strings.TrimSpace(a.Name) == "" || strings.TrimSpace(a.Command) == "" {
			return errors.New("an action needs a name and a command")
		}
		if a.On != "files" && a.On != "folders" && a.On != "any" {
			return fmt.Errorf("%q: on must be files, folders or any", a.Name)
		}
	}
	return nil
}

// checkZones checks a zone grid as the compositor does (layout/snap.rs):
// 1-16 zones, each inside the area and at least a tenth of it each way,
// none overlapping.
func checkZones(zones [][4]float64) error {
	const eps = 1e-6
	if len(zones) == 0 || len(zones) > 16 {
		return errors.New("must have 1-16 zones")
	}
	for i, z := range zones {
		x, y, w, h := z[0], z[1], z[2], z[3]
		if x < -eps || y < -eps || w < 0.1-eps || h < 0.1-eps || x+w > 1+eps || y+h > 1+eps {
			return fmt.Errorf("zone %d is outside the area or smaller than a tenth", i+1)
		}
		for j, o := range zones[:i] {
			ow := math.Min(x+w, o[0]+o[2]) - math.Max(x, o[0])
			oh := math.Min(y+h, o[1]+o[3]) - math.Max(y, o[1])
			if ow > eps && oh > eps {
				return fmt.Errorf("zones %d and %d overlap", j+1, i+1)
			}
		}
	}
	return nil
}

// pointerSpeed: libinput's range.
func pointerSpeed(v float64) error {
	if v < -1 || v > 1 {
		return errors.New("must be -1 to 1")
	}
	return nil
}
