// Package memwatch decides what to do as memory runs out
// (docs/plans/memory-protection.md): how tight it is, which apps look like
// they're leaking, and which app to name. whiterosed reads the numbers
// (cmd/whiterosed/memory.go); this package only judges them, so it can be
// tested.
//
// Apps are the scopes in the user's app.slice: WhiteRose's own
// (app-whiterose-<id>-<pid>-<n>.scope, compositor/src/app_scope.rs), and
// GNOME's and Flatpak's (app-gnome-<id>-<pid>.scope, app-flatpak-<id>-<n>.scope).
package memwatch

import (
	"bufio"
	"sort"
	"strconv"
	"strings"
	"time"
)

// ---- apps -----------------------------------------------------------------------------

// App is one app scope and the memory it holds now.
type App struct {
	Unit  string `json:"unit"`
	ID    string `json:"id"` // its app id, as its launcher named it
	Bytes int64  `json:"bytes"`
}

// ParseUnit: an app scope's app id ("app-whiterose-org.gnome.Calculator-123-0.scope"
// → "org.gnome.Calculator"); ok false for anything else (slices, services).
func ParseUnit(unit string) (id string, ok bool) {
	// A terminal's tab (Ptyxis: ptyxis-spawn-<uuid>.scope; GNOME Terminal
	// and Console, through VTE: vte-spawn-<uuid>.scope) is the terminal: a
	// build run in one is the terminal's work, and is kept like it.
	for prefix, term := range spawnScopes {
		if strings.HasPrefix(unit, prefix) && strings.HasSuffix(unit, ".scope") {
			return term, true
		}
	}
	rest, ok := strings.CutPrefix(unit, "app-")
	if !ok {
		return "", false
	}
	rest, ok = strings.CutSuffix(rest, ".scope")
	if !ok {
		return "", false
	}
	// <launcher>-<id>-<number>[-<number>]: the launcher has no "-" (an id's
	// "-" is escaped as \x2d), the numbers are at the end.
	launcher, rest, ok := strings.Cut(rest, "-")
	if !ok {
		return "", false
	}
	parts := strings.Split(rest, "-")
	for len(parts) > 1 && isNumber(parts[len(parts)-1]) {
		parts = parts[:len(parts)-1]
	}
	id = unescape(strings.Join(parts, "-"))
	// Ghostty moves each terminal tab into its own scope
	// (app-ghostty-surface-transient-<pid>.scope): that's Ghostty, the
	// terminal, not an app called "surface-transient".
	if id == "surface-transient" {
		id = launcher
	}
	return id, id != ""
}

func isNumber(s string) bool {
	_, err := strconv.ParseUint(s, 10, 64)
	return err == nil
}

// unescape undoes systemd-escape's \xNN.
func unescape(s string) string {
	var b strings.Builder
	for i := 0; i < len(s); i++ {
		if s[i] == '\\' && i+3 < len(s) && s[i+1] == 'x' {
			if n, err := strconv.ParseUint(s[i+2:i+4], 16, 8); err == nil {
				b.WriteByte(byte(n))
				i += 3
				continue
			}
		}
		b.WriteByte(s[i])
	}
	return b.String()
}

// spawnScopes: terminals that put each tab in a scope of its own, by the
// scope's prefix, and the terminal's app id.
var spawnScopes = map[string]string{
	"ptyxis-spawn-": "org.gnome.Ptyxis",
	"vte-spawn-":    "vte-terminal",
}

// Terminals are never closed, whichever is the default: what runs in them
// (a build, an editor, a session like this one) is work that closing the
// window would lose, and their own memory is small. Matched like
// memory.protected (the last part of a reverse-DNS id, without case).
var Terminals = []string{
	"org.gnome.Ptyxis", "org.gnome.Console", "kgx", "gnome-terminal", "org.gnome.Terminal",
	"vte-terminal", "org.kde.konsole", "kitty", "alacritty", "foot", "footclient",
	"org.wezfurlong.wezterm", "com.mitchellh.ghostty", "xterm", "com.gexperts.Tilix",
	"terminator", "io.elementary.terminal", "blackbox", "com.raggesilver.BlackBox",
}

// IsTerminal: app id is a known terminal (see Terminals).
func IsTerminal(id string) bool { return Protected(id, Terminals, "") }

// Protected: in `list` (memory.protected) or the terminal, matched as the
// compositor matches (app_scope.rs is_protected): without case, and by
// the last part of a reverse-DNS id.
func Protected(id string, list []string, terminal string) bool {
	short := func(s string) string {
		if i := strings.LastIndexByte(s, '.'); i >= 0 {
			s = s[i+1:]
		}
		return strings.ToLower(s)
	}
	names := append([]string(nil), list...)
	if f := strings.Fields(terminal); len(f) > 0 {
		p := f[0]
		if i := strings.LastIndexByte(p, '/'); i >= 0 {
			p = p[i+1:]
		}
		names = append(names, p)
	}
	for _, n := range names {
		if n != "" && (strings.EqualFold(n, id) || short(n) == short(id)) {
			return true
		}
	}
	return false
}

// OwnBytes is an app scope's own memory from its memory.stat: "anon", what
// it allocated and holds in RAM, so what closing it frees. Not the page
// cache, nor shared memory (files in a RAM /tmp) that can be charged to a
// scope it doesn't belong to (a swapoff run there brings it back) and that
// closing the app wouldn't free. ok false: no anon line.
func OwnBytes(stat string) (n int64, ok bool) {
	sc := bufio.NewScanner(strings.NewReader(stat))
	for sc.Scan() {
		if v, found := strings.CutPrefix(sc.Text(), "anon "); found {
			n, err := strconv.ParseInt(strings.TrimSpace(v), 10, 64)
			return n, err == nil
		}
	}
	return 0, false
}

// ---- pressure --------------------------------------------------------------------------

// PSI is /proc/pressure/memory's averages (% of time stalled, last 10 s).
type PSI struct {
	Some10, Full10 float64
}

func ParsePSI(text string) PSI {
	var p PSI
	sc := bufio.NewScanner(strings.NewReader(text))
	for sc.Scan() {
		f := strings.Fields(sc.Text())
		if len(f) < 2 {
			continue
		}
		v, ok := strings.CutPrefix(f[1], "avg10=")
		if !ok {
			continue
		}
		n, _ := strconv.ParseFloat(v, 64)
		switch f[0] {
		case "some":
			p.Some10 = n
		case "full":
			p.Full10 = n
		}
	}
	return p
}

type Level int

const (
	Calm Level = iota
	// Tight: things are waiting on memory, or little is left. The island
	// names the worst app and offers to close it.
	Tight
	// Critical: everything is stalling, or almost nothing is left. The
	// worst unprotected app is closed (step 4).
	Critical
)

func (l Level) String() string { return [...]string{"calm", "tight", "critical"}[l] }

// Judge: how tight memory is, from the pressure and what's available.
//
// Stalling alone is only Tight: a big parallel build (16 linkers at 1 GB
// each) stalls everything for a while with most of memory still free, and
// closing an app then frees nothing that matters (2026-11-14: Resources
// and the terminal running the build were closed). Critical, where an app
// is closed by itself, needs memory to be short as well.
func Judge(p PSI, availBytes, totalBytes int64) Level {
	frac := 1.0
	if totalBytes > 0 {
		frac = float64(availBytes) / float64(totalBytes)
	}
	switch {
	case frac < 0.03 || (p.Full10 >= 20 && frac < 0.15):
		return Critical
	case p.Full10 >= 20 || p.Some10 >= 10 || frac < 0.10:
		return Tight
	}
	return Calm
}

// WorthClosing: closing an app holding `bytes` would make a real
// difference: a tenth of memory, or 1 GB on machines with more than 10.
// Below that the pressure is coming from somewhere else (something not in
// an app scope, or many small things), and closing it only loses work.
func WorthClosing(bytes, totalBytes int64) bool {
	return bytes >= min(totalBytes/10, 1<<30)
}

// ---- leaks -------------------------------------------------------------------------------

type sample struct {
	at    time.Time
	bytes int64
}

// Tracker keeps each app's memory over the last minutes.
type Tracker struct {
	hist map[string][]sample
}

// Keep is how far back the tracker looks (a leak grows across it).
const Keep = 12 * time.Minute

// Add records this reading; apps that are gone are forgotten.
func (t *Tracker) Add(now time.Time, apps []App) {
	if t.hist == nil {
		t.hist = map[string][]sample{}
	}
	seen := map[string]bool{}
	for _, a := range apps {
		seen[a.Unit] = true
		h := append(t.hist[a.Unit], sample{now, a.Bytes})
		for len(h) > 0 && now.Sub(h[0].at) > Keep {
			h = h[1:]
		}
		t.hist[a.Unit] = h
	}
	for u := range t.hist {
		if !seen[u] {
			delete(t.hist, u)
		}
	}
}

// Growing: memory up steadily for 10 minutes: readings spanning at least
// 9.5 of them, at least 80% of the steps not going down, up by a quarter
// and at least 200 MB overall.
func (t *Tracker) Growing(unit string) bool {
	h := t.hist[unit]
	if len(h) < 6 || h[len(h)-1].at.Sub(h[0].at) < 9*time.Minute+30*time.Second {
		return false
	}
	up := 0
	for i := 1; i < len(h); i++ {
		if h[i].bytes >= h[i-1].bytes {
			up++
		}
	}
	first, last := h[0].bytes, h[len(h)-1].bytes
	return float64(up) >= 0.8*float64(len(h)-1) && last >= first+first/4 && last-first >= 200<<20
}

// Suspect is an app that looks like the problem.
type Suspect struct {
	App
	Growing bool   `json:"growing"` // up steadily for 10 minutes
	Reason  string `json:"reason"`  // "growing" | "half"
}

// Suspects: apps growing steadily while not focused, or holding more than
// half of memory; protected apps never; largest first. `focused`: the app
// id with the keyboard (a big document being worked on isn't a leak).
func (t *Tracker) Suspects(apps []App, total int64, focused string, protected func(id string) bool) []Suspect {
	var out []Suspect
	for _, a := range apps {
		if protected(a.ID) {
			continue
		}
		g := t.Growing(a.Unit) && !strings.EqualFold(a.ID, focused)
		switch {
		case g:
			out = append(out, Suspect{App: a, Growing: true, Reason: "growing"})
		case total > 0 && a.Bytes > total/2:
			out = append(out, Suspect{App: a, Reason: "half"})
		}
	}
	sort.Slice(out, func(i, j int) bool { return out[i].Bytes > out[j].Bytes })
	return out
}

// Worst is the app to name when memory is tight: the biggest suspect,
// else the biggest unprotected app (ok false if there's none).
func (t *Tracker) Worst(apps []App, total int64, focused string, protected func(id string) bool) (Suspect, bool) {
	if s := t.Suspects(apps, total, focused, protected); len(s) > 0 {
		return s[0], true
	}
	var best *App
	for i := range apps {
		if !protected(apps[i].ID) && (best == nil || apps[i].Bytes > best.Bytes) {
			best = &apps[i]
		}
	}
	if best == nil {
		return Suspect{}, false
	}
	return Suspect{App: *best, Growing: t.Growing(best.Unit), Reason: "biggest"}, true
}
