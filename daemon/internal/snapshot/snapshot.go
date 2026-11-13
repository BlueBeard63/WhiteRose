// Package snapshot takes what a bug report needs to know about the moment
// something went wrong: WhiteRose's version, the system (distribution,
// kernel, CPU, memory, GPUs, monitors), the load (memory, swap, the
// processes using the most CPU and memory), the windows open, and the end
// of the session log.
//
// Kept private on purpose: process names only (no command lines: they can
// hold paths and tokens), app IDs only (no window titles: they can hold
// document names, chats, pages). Nothing is sent anywhere from here; the
// report flow shows every section before anything leaves the computer.
package snapshot

import (
	"bufio"
	"encoding/json"
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"strconv"
	"strings"
	"time"
)

type Snapshot struct {
	Taken int64 `json:"taken"` // unix ms
	// Late: taken after the fact (the compositor crashed, so this is from
	// the next login): no load or windows from the moment itself.
	Late      bool      `json:"late,omitempty"`
	WhiteRose WhiteRose `json:"whiterose"`
	System    System    `json:"system"`
	Load      *Load     `json:"load,omitempty"`
	Windows   *Windows  `json:"windows,omitempty"`
	Log       []string  `json:"log"` // the session log's last lines
}

type WhiteRose struct {
	Version string `json:"version"`
	// How long the session had been running (s), from the session log's
	// first line; 0 if unknown.
	Uptime   int64 `json:"uptime"`
	Xwayland bool  `json:"xwayland"`
}

type System struct {
	OS       string    `json:"os"`
	Kernel   string    `json:"kernel"`
	CPU      string    `json:"cpu"`
	Threads  int       `json:"threads"`
	MemoryMB int64     `json:"memory_mb"`
	GPUs     []GPU     `json:"gpus"`
	Monitors []Monitor `json:"monitors"`
}

type GPU struct {
	Vendor string `json:"vendor"` // "Intel", "AMD", "NVIDIA" or the PCI id
	Device string `json:"device"` // PCI device id
	Driver string `json:"driver"`
}

type Monitor struct {
	Name    string  `json:"name"` // make and model
	Mode    string  `json:"mode"` // "2560x1440 @ 165 Hz"
	Scale   float64 `json:"scale"`
	Builtin bool    `json:"builtin,omitempty"`
}

type Load struct {
	Avg        [3]float64 `json:"avg"` // 1, 5, 15 min
	MemUsedMB  int64      `json:"mem_used_mb"`
	MemTotalMB int64      `json:"mem_total_mb"`
	SwapUsedMB int64      `json:"swap_used_mb"`
	TopCPU     []Process  `json:"top_cpu"`
	TopMemory  []Process  `json:"top_memory"`
}

type Process struct {
	Name  string  `json:"name"`
	CPU   float64 `json:"cpu"` // % of one core, over the sample
	RSSMB int64   `json:"rss_mb"`
}

type Windows struct {
	Count        int            `json:"count"`
	PerWorkspace map[int]int    `json:"per_workspace"`
	Apps         map[string]int `json:"apps"` // app ID → windows
	Focused      string         `json:"focused"`
}

// Sources is where it looks (tests point them elsewhere).
type Sources struct {
	Proc       string // /proc
	Sys        string // /sys
	OSRelease  string // /etc/os-release
	SessionLog string // ~/.local/state/whiterose/session.log
	Version    string
	// Compositor returns its windows and outputs events (raw), or nil
	// when there's no compositor to ask.
	Compositor func() (windows, outputs json.RawMessage)
	Sample     time.Duration // between the two CPU readings (500 ms)
	LogLines   int           // 400
	Now        func() time.Time
}

func (s Sources) now() time.Time {
	if s.Now != nil {
		return s.Now()
	}
	return time.Now()
}

// Take a snapshot. `late`: the moment has passed (see Snapshot.Late).
func Take(src Sources, late bool) Snapshot {
	if src.Proc == "" {
		src.Proc = "/proc"
	}
	if src.Sys == "" {
		src.Sys = "/sys"
	}
	if src.OSRelease == "" {
		src.OSRelease = "/etc/os-release"
	}
	if src.LogLines == 0 {
		src.LogLines = 400
	}
	snap := Snapshot{Taken: src.now().UnixMilli(), Late: late}
	snap.WhiteRose = WhiteRose{Version: src.Version, Xwayland: running(src.Proc, "Xwayland")}
	snap.System = system(src)
	var windows, outputs json.RawMessage
	if src.Compositor != nil {
		windows, outputs = src.Compositor()
	}
	snap.System.Monitors = monitors(outputs)
	if !late {
		snap.Load = load(src)
		snap.Windows = parseWindows(windows)
	}
	snap.Log, snap.WhiteRose.Uptime = sessionLog(src.SessionLog, src.LogLines, src.now())
	return snap
}

// ---- the system ---------------------------------------------------------------------

func system(src Sources) System {
	s := System{Threads: runtime.NumCPU(), GPUs: []GPU{}, Monitors: []Monitor{}}
	s.OS = osRelease(src.OSRelease)
	if b, err := os.ReadFile(filepath.Join(src.Proc, "sys/kernel/osrelease")); err == nil {
		s.Kernel = strings.TrimSpace(string(b))
	}
	if b, err := os.ReadFile(filepath.Join(src.Proc, "cpuinfo")); err == nil {
		for l := range strings.SplitSeq(string(b), "\n") {
			if k, v, ok := strings.Cut(l, ":"); ok && strings.TrimSpace(k) == "model name" {
				s.CPU = strings.TrimSpace(v)
				break
			}
		}
	}
	mem := meminfo(src.Proc)
	s.MemoryMB = mem["MemTotal"] / 1024
	s.GPUs = gpus(src.Sys)
	return s
}

func osRelease(path string) string {
	b, err := os.ReadFile(path)
	if err != nil {
		return ""
	}
	for l := range strings.SplitSeq(string(b), "\n") {
		if v, ok := strings.CutPrefix(l, "PRETTY_NAME="); ok {
			return strings.Trim(v, `"`)
		}
	}
	return ""
}

// meminfo in kB.
func meminfo(proc string) map[string]int64 {
	out := map[string]int64{}
	b, err := os.ReadFile(filepath.Join(proc, "meminfo"))
	if err != nil {
		return out
	}
	for l := range strings.SplitSeq(string(b), "\n") {
		k, v, ok := strings.Cut(l, ":")
		if !ok {
			continue
		}
		f := strings.Fields(v)
		if len(f) > 0 {
			n, _ := strconv.ParseInt(f[0], 10, 64)
			out[k] = n
		}
	}
	return out
}

var vendors = map[string]string{"0x8086": "Intel", "0x1002": "AMD", "0x10de": "NVIDIA", "0x1af4": "Virtio", "0x15ad": "VMware"}

func gpus(sys string) []GPU {
	out := []GPU{}
	cards, _ := filepath.Glob(filepath.Join(sys, "class/drm/card[0-9]*"))
	seen := map[string]bool{}
	for _, c := range cards {
		if strings.Contains(filepath.Base(c), "-") { // card0-HDMI-A-1: a connector
			continue
		}
		dev := filepath.Join(c, "device")
		real, _ := filepath.EvalSymlinks(dev)
		if seen[real] {
			continue
		}
		seen[real] = true
		read := func(n string) string {
			b, _ := os.ReadFile(filepath.Join(dev, n))
			return strings.TrimSpace(string(b))
		}
		g := GPU{Vendor: read("vendor"), Device: read("device")}
		if v, ok := vendors[g.Vendor]; ok {
			g.Vendor = v
		}
		if d, err := os.Readlink(filepath.Join(dev, "driver")); err == nil {
			g.Driver = filepath.Base(d)
		}
		out = append(out, g)
	}
	return out
}

func monitors(raw json.RawMessage) []Monitor {
	out := []Monitor{}
	var ev struct {
		Outputs []struct {
			Make, Model string
			Builtin     bool
			Enabled     bool
			Scale       float64
			Mode        *struct {
				Width, Height int
				Refresh       int // mHz
			}
		}
	}
	if json.Unmarshal(raw, &ev) != nil {
		return out
	}
	for _, o := range ev.Outputs {
		if !o.Enabled {
			continue
		}
		m := Monitor{Name: strings.TrimSpace(o.Make + " " + o.Model), Scale: o.Scale, Builtin: o.Builtin}
		if o.Mode != nil {
			m.Mode = strconv.Itoa(o.Mode.Width) + "x" + strconv.Itoa(o.Mode.Height) +
				" @ " + strconv.FormatFloat(float64(o.Mode.Refresh)/1000, 'f', -1, 64) + " Hz"
		}
		out = append(out, m)
	}
	return out
}

// ---- the load ------------------------------------------------------------------------

func load(src Sources) *Load {
	l := &Load{TopCPU: []Process{}, TopMemory: []Process{}}
	if b, err := os.ReadFile(filepath.Join(src.Proc, "loadavg")); err == nil {
		f := strings.Fields(string(b))
		for i := 0; i < 3 && i < len(f); i++ {
			l.Avg[i], _ = strconv.ParseFloat(f[i], 64)
		}
	}
	mem := meminfo(src.Proc)
	l.MemTotalMB = mem["MemTotal"] / 1024
	l.MemUsedMB = (mem["MemTotal"] - mem["MemAvailable"]) / 1024
	l.SwapUsedMB = (mem["SwapTotal"] - mem["SwapFree"]) / 1024

	sample := src.Sample
	if sample == 0 {
		sample = 500 * time.Millisecond
	}
	before := processes(src.Proc)
	time.Sleep(sample)
	after := processes(src.Proc)
	tick := 100.0 // USER_HZ: 100 on Linux everywhere WhiteRose runs
	var all []Process
	for pid, a := range after {
		p := Process{Name: a.name, RSSMB: a.rssKB / 1024}
		if b, ok := before[pid]; ok && a.ticks >= b.ticks {
			p.CPU = float64(a.ticks-b.ticks) / tick / sample.Seconds() * 100
			p.CPU = float64(int(p.CPU*10+0.5)) / 10
		}
		all = append(all, p)
	}
	l.TopCPU = top(all, func(a, b Process) bool { return a.CPU > b.CPU }, func(p Process) bool { return p.CPU > 0 })
	l.TopMemory = top(all, func(a, b Process) bool { return a.RSSMB > b.RSSMB }, func(p Process) bool { return p.RSSMB > 0 })
	return l
}

func top(all []Process, more func(a, b Process) bool, keep func(Process) bool) []Process {
	ps := append([]Process(nil), all...)
	sort.SliceStable(ps, func(i, j int) bool {
		if more(ps[i], ps[j]) != more(ps[j], ps[i]) {
			return more(ps[i], ps[j])
		}
		return ps[i].Name < ps[j].Name
	})
	out := []Process{}
	for _, p := range ps {
		if len(out) == 10 {
			break
		}
		if keep(p) {
			out = append(out, p)
		}
	}
	return out
}

type proc struct {
	name  string
	ticks uint64
	rssKB int64
}

func processes(root string) map[int]proc {
	out := map[int]proc{}
	entries, _ := os.ReadDir(root)
	page := int64(os.Getpagesize() / 1024)
	for _, e := range entries {
		pid, err := strconv.Atoi(e.Name())
		if err != nil {
			continue
		}
		b, err := os.ReadFile(filepath.Join(root, e.Name(), "stat"))
		if err != nil {
			continue
		}
		// pid (comm) state ppid ...: comm may hold spaces and brackets.
		s := string(b)
		open, end := strings.IndexByte(s, '('), strings.LastIndexByte(s, ')')
		if open < 0 || end < open {
			continue
		}
		f := strings.Fields(s[end+1:])
		// After comm: state(0) ... utime(11) stime(12) ... rss(21).
		if len(f) < 22 {
			continue
		}
		ut, _ := strconv.ParseUint(f[11], 10, 64)
		st, _ := strconv.ParseUint(f[12], 10, 64)
		rss, _ := strconv.ParseInt(f[21], 10, 64)
		out[pid] = proc{name: s[open+1 : end], ticks: ut + st, rssKB: rss * page}
	}
	return out
}

func running(root, name string) bool {
	for _, p := range processes(root) {
		if p.name == name {
			return true
		}
	}
	return false
}

// ---- windows and the log ---------------------------------------------------------------

func parseWindows(raw json.RawMessage) *Windows {
	var ev struct {
		Windows []struct {
			AppID     *string `json:"app_id"`
			Workspace int     `json:"workspace"`
			Focused   bool    `json:"focused"`
		} `json:"windows"`
	}
	if raw == nil || json.Unmarshal(raw, &ev) != nil {
		return nil
	}
	w := &Windows{Count: len(ev.Windows), PerWorkspace: map[int]int{}, Apps: map[string]int{}}
	for _, x := range ev.Windows {
		id := "(no app ID)"
		if x.AppID != nil && *x.AppID != "" {
			id = *x.AppID
		}
		w.PerWorkspace[x.Workspace]++
		w.Apps[id]++
		if x.Focused {
			w.Focused = id
		}
	}
	return w
}

// sessionLog: its last `n` lines, and how long ago it started (from its
// first line, "WhiteRose 0.3.0, session started 2026-11-13 12:10:07").
func sessionLog(path string, n int, now time.Time) ([]string, int64) {
	f, err := os.Open(path)
	if err != nil {
		return []string{}, 0
	}
	defer f.Close()
	var lines []string
	var uptime int64
	sc := bufio.NewScanner(f)
	sc.Buffer(make([]byte, 64<<10), 1<<20)
	first := true
	for sc.Scan() {
		l := sc.Text()
		if first {
			first = false
			if _, at, ok := strings.Cut(l, "session started "); ok {
				if t, err := time.ParseInLocation(time.DateTime, at, time.Local); err == nil {
					uptime = int64(now.Sub(t).Seconds())
				}
			}
		}
		lines = append(lines, l)
		if len(lines) > 2*n {
			lines = append([]string(nil), lines[len(lines)-n:]...)
		}
	}
	if sc.Err() != nil {
		lines = append(lines, "(the rest of the log couldn't be read: "+sc.Err().Error()+")")
	}
	if len(lines) > n {
		lines = lines[len(lines)-n:]
	}
	if lines == nil {
		lines = []string{}
	}
	return lines, uptime
}
