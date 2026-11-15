// Package report turns a bug report draft (the island's steps) and its
// snapshot into the report file and the prefilled GitHub bug form
// (.github/ISSUE_TEMPLATE/bug.yml). Only the sections left switched on in
// the last step go in either.
package report

import (
	"encoding/json"
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"slices"
	"sort"
	"strings"
	"time"

	"whiterose/daemon/internal/crash"
	"whiterose/daemon/internal/snapshot"
)

// Draft is what the steps collect (the island keeps it in whiterosed, so
// a shell restart doesn't lose it).
type Draft struct {
	Step     int    `json:"step"`
	Snapshot string `json:"snapshot"` // crash record ID
	Kind     string `json:"kind"`     // crashed | froze | looks | wrong | slow
	Where    string `json:"where"`    // the area's label ("Files", "The bar")
	Title    string `json:"title"`
	Edited   bool   `json:"edited"` // the title was changed by hand (kept)
	What     string `json:"what"`
	// Sections switched off in the last step (on unless listed).
	Off []string `json:"off"`
}

// Sections, in the order they're shown and written.
var Sections = []string{"whiterose", "system", "load", "windows", "output", "log"}

func (d Draft) on(section string) bool { return !slices.Contains(d.Off, section) }

var kinds = map[string]string{
	"crashed": "Crashed", "froze": "Froze or stopped responding", "looks": "Looks wrong",
	"wrong": "Doesn't do what it should", "slow": "Slow",
}

// Input is everything a report is made from.
type Input struct {
	Draft    Draft
	Record   crash.Record
	Snap     *snapshot.Snapshot // nil if it couldn't be taken
	Output   string             // what the crashed piece printed last
	Build    string             // update.Build* state
	BuildID  string
	Repo     string // where the form is ("" for none: a file only)
	Now      time.Time
	MaxLines int // log lines in the file (400)
}

// File is the report file's text.
func File(in Input) string {
	var b strings.Builder
	d := in.Draft
	fmt.Fprintf(&b, "WhiteRose bug report, %s\n\n", in.Now.Format(time.DateTime))
	fmt.Fprintf(&b, "Title: %s\n", d.Title)
	fmt.Fprintf(&b, "Kind: %s\n", kinds[d.Kind])
	fmt.Fprintf(&b, "Where: %s\n", d.Where)
	fmt.Fprintf(&b, "Problem: %s (%s)\n", in.Record.Describe(), time.UnixMilli(in.Record.Time).Format(time.DateTime))
	fmt.Fprintf(&b, "Build: %s %s\n", in.Build, in.BuildID)
	fmt.Fprintf(&b, "\nWhat happened\n-------------\n%s\n", strings.TrimSpace(d.What))
	for _, s := range Sections {
		if !d.on(s) {
			continue
		}
		text := section(in, s, false)
		if text == "" {
			continue
		}
		title := sectionTitles[s]
		fmt.Fprintf(&b, "\n%s\n%s\n%s\n", title, strings.Repeat("-", len(title)), text)
	}
	return b.String()
}

var sectionTitles = map[string]string{
	"whiterose": "WhiteRose", "system": "System", "load": "Load at the time",
	"windows": "Windows", "output": "What it printed last", "log": "Session log (end)",
}

// Section is one section's text as the last step shows it.
func Section(in Input, s string) string { return section(in, s, false) }

func section(in Input, s string, short bool) string {
	snap := in.Snap
	var b strings.Builder
	switch s {
	case "whiterose":
		if snap == nil {
			return ""
		}
		w := snap.WhiteRose
		fmt.Fprintf(&b, "Version %s", w.Version)
		if w.Uptime > 0 {
			fmt.Fprintf(&b, ", session up %s", uptime(w.Uptime))
		}
		fmt.Fprintf(&b, ", Xwayland %s", map[bool]string{true: "running", false: "not running"}[w.Xwayland])
		if snap.Late {
			b.WriteString("\n(Taken at the next login: the load and windows at the moment itself aren't known.)")
		}
	case "system":
		if snap == nil {
			return ""
		}
		y := snap.System
		fmt.Fprintf(&b, "%s, kernel %s\n%s, %d threads, %d MB memory", y.OS, y.Kernel, y.CPU, y.Threads, y.MemoryMB)
		for _, g := range y.GPUs {
			fmt.Fprintf(&b, "\nGPU: %s %s (%s)", g.Vendor, g.Device, g.Driver)
		}
		for _, m := range y.Monitors {
			fmt.Fprintf(&b, "\nMonitor: %s, %s, scale %g", m.Name, m.Mode, m.Scale)
		}
	case "load":
		if snap == nil || snap.Load == nil {
			return ""
		}
		l := snap.Load
		fmt.Fprintf(&b, "Load %.2f %.2f %.2f; memory %d of %d MB used; swap %d MB used",
			l.Avg[0], l.Avg[1], l.Avg[2], l.MemUsedMB, l.MemTotalMB, l.SwapUsedMB)
		if !short {
			b.WriteString("\nMost CPU:")
			for _, p := range l.TopCPU {
				fmt.Fprintf(&b, "\n  %-24s %5.1f%%  %6d MB", fit(p.Name, 24), p.CPU, p.RSSMB)
			}
			b.WriteString("\nMost memory:")
			for _, p := range l.TopMemory {
				fmt.Fprintf(&b, "\n  %-24s %6d MB", fit(p.Name, 24), p.RSSMB)
			}
		}
	case "windows":
		if snap == nil || snap.Windows == nil {
			return ""
		}
		w := snap.Windows
		fmt.Fprintf(&b, "%d open", w.Count)
		if w.Focused != "" {
			fmt.Fprintf(&b, ", focused: %s", w.Focused)
		}
		apps := make([]string, 0, len(w.Apps))
		for a, n := range w.Apps {
			apps = append(apps, fmt.Sprintf("%s ×%d", a, n))
		}
		sort.Strings(apps)
		if len(apps) > 0 {
			b.WriteString("\n")
			b.WriteString(strings.Join(apps, ", "))
		}
	case "output":
		if strings.TrimSpace(in.Output) == "" {
			return ""
		}
		b.WriteString(strings.TrimRight(in.Output, "\n"))
	case "log":
		if snap == nil || len(snap.Log) == 0 {
			return ""
		}
		lines := snap.Log
		if n := in.MaxLines; n > 0 && len(lines) > n {
			lines = lines[len(lines)-n:]
		}
		b.WriteString(strings.Join(lines, "\n"))
	}
	return b.String()
}

// uptime: "45 min", "1 h 5 min", "2 d 3 h".
func uptime(s int64) string {
	m, h, d := s/60%60, s/3600%24, s/86400
	switch {
	case d > 0:
		return fmt.Sprintf("%d d %d h", d, h)
	case h > 0:
		return fmt.Sprintf("%d h %d min", h, m)
	}
	return fmt.Sprintf("%d min", m)
}

// fit cuts a name to n characters ("…" at the end).
func fit(s string, n int) string {
	r := []rune(s)
	if len(r) <= n {
		return s
	}
	return string(r[:n-1]) + "…"
}

// FormURL is the repository's bug form, prefilled (fields by their id in
// bug.yml). Long fields are cut so the address stays short enough for a
// browser; the whole thing is in the file to attach.
func FormURL(in Input, fileName string) string {
	d := in.Draft
	cut := func(s string, n int) string {
		s = strings.TrimSpace(s)
		if len(s) > n {
			return s[:n] + "… (the rest is in " + fileName + ")"
		}
		return s
	}
	var sys []string
	for _, s := range []string{"whiterose", "system", "windows"} {
		if d.on(s) {
			if t := section(in, s, true); t != "" {
				sys = append(sys, t)
			}
		}
	}
	if d.on("load") {
		if t := section(in, "load", true); t != "" {
			sys = append(sys, t)
		}
	}
	q := url.Values{}
	q.Set("template", "bug.yml")
	q.Set("title", d.Title)
	version := ""
	if in.Snap != nil {
		version = in.Snap.WhiteRose.Version
	}
	q.Set("version", version)
	q.Set("build", in.BuildID)
	q.Set("what", cut(kinds[d.Kind]+" · "+d.Where+"\n\n"+d.What, 2500))
	q.Set("system", cut(strings.Join(sys, "\n\n"), 2000))
	q.Set("logs", "Attached: "+fileName)
	return "https://github.com/" + in.Repo + "/issues/new?" + q.Encode()
}

// Load reads a snapshot.json.
func Load(b []byte) *snapshot.Snapshot {
	var s snapshot.Snapshot
	if json.Unmarshal(b, &s) != nil {
		return nil
	}
	return &s
}

// DownloadsDir is where report files go: XDG_DOWNLOAD_DIR from
// ~/.config/user-dirs.dirs, else ~/Downloads.
func DownloadsDir(home string) string {
	if b, err := os.ReadFile(filepath.Join(home, ".config", "user-dirs.dirs")); err == nil {
		for l := range strings.SplitSeq(string(b), "\n") {
			if v, ok := strings.CutPrefix(strings.TrimSpace(l), "XDG_DOWNLOAD_DIR="); ok {
				v = strings.Trim(v, `"`)
				v = strings.Replace(v, "$HOME", home, 1)
				if filepath.IsAbs(v) && v != home {
					return v
				}
			}
		}
	}
	return filepath.Join(home, "Downloads")
}
