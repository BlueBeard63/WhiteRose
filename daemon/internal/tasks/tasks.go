// Package tasks samples Linux performance counters on demand. A shared sampler
// keeps one minute of history without polling when no Task manager is open.
package tasks

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"sync"
	"time"

	"golang.org/x/sys/unix"
	"whiterose/daemon/internal/memwatch"
)

type Process struct {
	PID    int      `json:"pid"`
	Name   string   `json:"name"`
	State  string   `json:"state"`
	Start  uint64   `json:"start"`
	CPU    *float64 `json:"cpu"`
	Memory uint64   `json:"memory"`
	ticks  uint64
}
type App struct {
	Unit                 string    `json:"unit"`
	ID                   string    `json:"id"`
	Token                string    `json:"token"`
	Memory               *uint64   `json:"memory"`
	CPU                  *float64  `json:"cpu"`
	Read                 *float64  `json:"read"`
	Write                *float64  `json:"write"`
	Processes            []Process `json:"processes"`
	CanEnd               bool      `json:"can_end"`
	Reason               string    `json:"reason"`
	usage, read, written uint64
	cpuOK, ioOK          bool
}
type Series struct {
	ID           string     `json:"id"`
	Value        *float64   `json:"value"`
	Other        *float64   `json:"other"`
	Total        *uint64    `json:"total,omitempty"`
	Used         *uint64    `json:"used,omitempty"`
	History      []*float64 `json:"history"`
	OtherHistory []*float64 `json:"other_history"`
}
type Snapshot struct {
	Time    int64    `json:"time"`
	Apps    []App    `json:"apps"`
	CPU     []Series `json:"cpu"`
	Memory  Series   `json:"memory"`
	Disks   []Series `json:"disks"`
	Network []Series `json:"network"`
	GPU     []Series `json:"gpu"`
	Scoped  bool     `json:"scoped"`
}
type counter struct{ a, b uint64 }
type Sampler struct {
	Proc, Sys, Slice string
	UID              int
	mu               sync.Mutex
	last             time.Time
	state            Snapshot
	counters         map[string]counter
}

func New(slice string) *Sampler {
	// A fixture (uitests) can point the sampler at a made-up /proc, /sys and
	// app.slice, so nothing real is read, named or ended.
	proc, sys := "/proc", "/sys"
	if d := os.Getenv("WHITEROSE_TASKS_PROC"); d != "" {
		proc = d
	}
	if d := os.Getenv("WHITEROSE_TASKS_SYS"); d != "" {
		sys = d
	}
	if d := os.Getenv("WHITEROSE_TASKS_SLICE"); d != "" {
		slice = d
	}
	return &Sampler{Proc: proc, Sys: sys, Slice: slice, UID: os.Getuid()}
}
func read(path string) string { b, _ := os.ReadFile(path); return string(b) }
func number(s string) uint64  { n, _ := strconv.ParseUint(strings.TrimSpace(s), 10, 64); return n }
func ptr(v float64) *float64  { return &v }
func fields(text string) map[string]uint64 {
	m := map[string]uint64{}
	for line := range strings.SplitSeq(text, "\n") {
		f := strings.Fields(line)
		if len(f) >= 2 {
			m[strings.TrimSuffix(f[0], ":")] = number(f[1])
		}
	}
	return m
}
func delta(a, b uint64) (uint64, bool) {
	if a < b {
		return 0, false
	}
	return a - b, true
}
func rate(now, old uint64, seconds float64, ok bool) *float64 {
	if d, valid := delta(now, old); ok && valid && seconds > 0 {
		return ptr(float64(d) / seconds)
	}
	return nil
}
func history(old []*float64, value *float64) []*float64 {
	start := max(0, len(old)-59)
	out := append([]*float64{}, old[start:]...)
	return append(out, value)
}
func oldSeries(list []Series, id string) Series {
	for _, s := range list {
		if s.ID == id {
			return s
		}
	}
	return Series{ID: id}
}
func (s *Sampler) series(id string, a, b *float64, old []Series) Series {
	o := oldSeries(old, id)
	return Series{ID: id, Value: a, Other: b, History: history(o.History, a), OtherHistory: history(o.OtherHistory, b)}
}

// Sample is serialized and cached for one second across all clients.
func (s *Sampler) Sample() Snapshot {
	s.mu.Lock()
	defer s.mu.Unlock()
	now := time.Now()
	if !s.last.IsZero() && now.Sub(s.last) < time.Second {
		return s.state
	}
	dt := now.Sub(s.last).Seconds()
	old := s.state
	next := Snapshot{Time: now.UnixMilli(), Apps: []App{}, CPU: []Series{}, Disks: []Series{}, Network: []Series{}, GPU: []Series{}}
	counters := map[string]counter{}
	totalDelta := uint64(0)
	cores := 0
	for line := range strings.SplitSeq(read(filepath.Join(s.Proc, "stat")), "\n") {
		f := strings.Fields(line)
		if len(f) < 5 || !strings.HasPrefix(f[0], "cpu") {
			continue
		}
		// Guest fields are already included in user/nice.
		var total uint64
		for _, v := range f[1:min(9, len(f))] {
			total += number(v)
		}
		idle := number(f[4])
		if len(f) > 5 {
			idle += number(f[5])
		}
		key := f[0]
		prev, ok := s.counters[key]
		counters[key] = counter{total, idle}
		var value *float64
		d, valid := delta(total, prev.a)
		i, iv := delta(idle, prev.b)
		if ok && valid && iv && d > 0 && i <= d {
			value = ptr(100 * float64(d-i) / float64(d))
		}
		if key == "cpu" {
			totalDelta = d
		} else {
			cores++
		}
		next.CPU = append(next.CPU, s.series(key, value, nil, old.CPU))
	}
	mem := fields(read(filepath.Join(s.Proc, "meminfo")))
	if total, ok := mem["MemTotal"]; ok && total > 0 {
		total *= 1024
		avail := min(total, mem["MemAvailable"]*1024)
		used := total - avail
		next.Memory = s.series("Memory", ptr(100*float64(used)/float64(total)), nil, []Series{old.Memory})
		next.Memory.Total = &total
		next.Memory.Used = &used
	}
	for line := range strings.SplitSeq(read(filepath.Join(s.Proc, "diskstats")), "\n") {
		f := strings.Fields(line)
		if len(f) < 14 {
			continue
		}
		id := f[2]
		// Whole block devices only; partitions would double count.
		if _, err := os.Stat(filepath.Join(s.Sys, "block", id)); err != nil {
			continue
		}
		if strings.HasPrefix(id, "loop") || strings.HasPrefix(id, "ram") {
			continue
		}
		key := "disk/" + id
		prev, ok := s.counters[key]
		a, b := number(f[5])*512, number(f[9])*512
		counters[key] = counter{a, b}
		next.Disks = append(next.Disks, s.series(id, rate(a, prev.a, dt, ok), rate(b, prev.b, dt, ok), old.Disks))
	}
	for line := range strings.SplitSeq(read(filepath.Join(s.Proc, "net/dev")), "\n") {
		id, rest, ok := strings.Cut(line, ":")
		if !ok {
			continue
		}
		id = strings.TrimSpace(id)
		f := strings.Fields(rest)
		if len(f) < 16 {
			continue
		}
		key := "net/" + id
		prev, ok := s.counters[key]
		a, b := number(f[0]), number(f[8])
		counters[key] = counter{a, b}
		next.Network = append(next.Network, s.series(id, rate(a, prev.a, dt, ok), rate(b, prev.b, dt, ok), old.Network))
	}
	cards, _ := filepath.Glob(filepath.Join(s.Sys, "class/drm/card[0-9]*"))
	for _, card := range cards {
		id := filepath.Base(card)
		if strings.Contains(id, "-") {
			continue
		}
		var value *float64
		if b, err := os.ReadFile(filepath.Join(card, "device/gpu_busy_percent")); err == nil {
			if n, err := strconv.ParseFloat(strings.TrimSpace(string(b)), 64); err == nil && n >= 0 && n <= 100 {
				value = ptr(n)
			}
		}
		g := s.series(id, value, nil, old.GPU)
		if b, err := os.ReadFile(filepath.Join(card, "device/mem_info_vram_total")); err == nil {
			v := number(string(b))
			g.Total = &v
		}
		if b, err := os.ReadFile(filepath.Join(card, "device/mem_info_vram_used")); err == nil {
			v := number(string(b))
			g.Used = &v
		}
		next.GPU = append(next.GPU, g)
	}
	seen := map[int]bool{}
	entries, err := os.ReadDir(s.Slice)
	next.Scoped = err == nil
	for _, e := range entries {
		id, ok := memwatch.ParseUnit(e.Name())
		if !ok || !e.IsDir() {
			continue
		}
		path := filepath.Join(s.Slice, e.Name())
		a := App{Unit: e.Name(), ID: id, Processes: []Process{}}
		a.Token, _ = scopeToken(path)
		ms := read(filepath.Join(path, "memory.stat"))
		if n, ok := memwatch.OwnBytes(ms); ok && n >= 0 {
			v := uint64(n)
			a.Memory = &v
		}
		if a.Memory == nil {
			if b, err := os.ReadFile(filepath.Join(path, "memory.current")); err == nil {
				v := number(string(b))
				a.Memory = &v
			}
		}
		cs := fields(read(filepath.Join(path, "cpu.stat")))
		a.usage, a.cpuOK = cs["usage_usec"]
		io, err := os.ReadFile(filepath.Join(path, "io.stat"))
		a.ioOK = err == nil
		for _, line := range strings.Split(string(io), "\n") {
			for _, v := range strings.Fields(line) {
				k, n, ok := strings.Cut(v, "=")
				if ok {
					switch k {
					case "rbytes":
						a.read += number(n)
					case "wbytes":
						a.written += number(n)
					}
				}
			}
		}
		for _, pid := range scopePIDs(path) {
			if p, ok := s.process(pid); ok {
				a.Processes = append(a.Processes, p)
				seen[pid] = true
			}
		}
		prev, ok := s.counters["app/"+a.Unit+"/"+a.Token]
		if a.cpuOK {
			a.CPU = rate(a.usage, prev.a, dt, ok)
			if a.CPU != nil {
				v := min(100, *a.CPU/10000/float64(max(1, cores)))
				a.CPU = &v
			}
		}
		counters["app/"+a.Unit+"/"+a.Token] = counter{a.usage, 0}
		prev, ok = s.counters["io/"+a.Unit+"/"+a.Token]
		a.Read = rate(a.read, prev.a, dt, ok && a.ioOK)
		a.Write = rate(a.written, prev.b, dt, ok && a.ioOK)
		if a.ioOK {
			counters["io/"+a.Unit+"/"+a.Token] = counter{a.read, a.written}
		}
		for i := range a.Processes {
			p := &a.Processes[i]
			prev, ok := s.counters[fmt.Sprintf("pid/%d/%d", p.PID, p.Start)]
			if d, v := delta(p.ticks, prev.a); ok && v && totalDelta > 0 {
				p.CPU = ptr(min(100, 100*float64(d)/float64(totalDelta)))
			}
			counters[fmt.Sprintf("pid/%d/%d", p.PID, p.Start)] = counter{p.ticks, 0}
		}
		a.Reason = s.readOnly(a)
		a.CanEnd = a.Reason == ""
		next.Apps = append(next.Apps, a)
	}
	// Processes outside recognized scopes are deliberately read-only.
	procs, _ := os.ReadDir(s.Proc)
	for _, e := range procs {
		pid, err := strconv.Atoi(e.Name())
		if err != nil || seen[pid] {
			continue
		}
		p, ok := s.process(pid)
		if !ok {
			continue
		}
		next.Apps = append(next.Apps, App{ID: p.Name, Unit: fmt.Sprintf("pid:%d", pid), Processes: []Process{p}, Reason: "Outside an app scope; read-only"})
	}
	sort.Slice(next.Apps, func(i, j int) bool { return next.Apps[i].Unit < next.Apps[j].Unit })
	s.counters = counters
	s.last = now
	s.state = next
	return next
}

func (s *Sampler) process(pid int) (Process, bool) {
	dir := filepath.Join(s.Proc, strconv.Itoa(pid))
	status := fields(read(filepath.Join(dir, "status")))
	uid, ok := status["Uid"]
	if !ok || int(uid) != s.UID {
		return Process{}, false
	}
	return parseProcess(pid, read(filepath.Join(dir, "stat")), status)
}
func parseProcess(pid int, stat string, status map[string]uint64) (Process, bool) {
	i, j := strings.IndexByte(stat, '('), strings.LastIndexByte(stat, ')')
	if i < 0 || j < i {
		return Process{}, false
	}
	f := strings.Fields(stat[j+1:])
	if len(f) < 22 {
		return Process{}, false
	}
	return Process{PID: pid, Name: stat[i+1 : j], State: f[0], Start: number(f[19]), ticks: number(f[11]) + number(f[12]), Memory: status["RssAnon"] * 1024}, true
}
func scopePIDs(path string) []int {
	out := []int{}
	seen := map[int]bool{}
	_ = filepath.WalkDir(path, func(p string, e os.DirEntry, err error) error {
		if err != nil {
			return filepath.SkipDir
		}
		if !e.IsDir() {
			return nil
		}
		for _, v := range strings.Fields(read(filepath.Join(p, "cgroup.procs"))) {
			if n, err := strconv.Atoi(v); err == nil && n > 1 && !seen[n] {
				out = append(out, n)
				seen[n] = true
			}
		}
		return nil
	})
	sort.Ints(out)
	return out
}
func scopeToken(path string) (string, error) {
	var st unix.Stat_t
	if err := unix.Lstat(path, &st); err != nil {
		return "", err
	}
	if st.Mode&unix.S_IFMT != unix.S_IFDIR {
		return "", errors.New("not a scope directory")
	}
	return fmt.Sprintf("%d:%d:%d:%d", st.Dev, st.Ino, st.Ctim.Sec, st.Ctim.Nsec), nil
}
func (s *Sampler) readOnly(a App) string {
	if len(a.Processes) == 0 {
		return "No controllable processes"
	}
	for _, pid := range scopePIDs(filepath.Join(s.Slice, a.Unit)) {
		p, ok := s.process(pid)
		if !ok {
			return "Contains processes outside your control"
		}
		switch p.Name {
		case "whiterosed", "whiterose-wm", "quickshell", "qs", "systemd", "dbus-daemon", "dbus-broker", "gnome-shell", "kwin_wayland", "Xorg", "Xwayland":
			return "Session component; read-only"
		}
		if pid == os.Getpid() {
			return "Session component; read-only"
		}
	}
	return ""
}

// End revalidates scope identity and every process, then uses pidfds so PID
// reuse cannot signal a different process. SIGTERM never schedules escalation.
// Force targets cgroup.kill via the held directory fd, never a process name.
func (s *Sampler) End(unit, token string, force bool) error {
	if _, ok := memwatch.ParseUnit(unit); !ok || filepath.Base(unit) != unit {
		return errors.New("not an app scope")
	}
	path := filepath.Join(s.Slice, unit)
	fd, err := unix.Open(path, unix.O_DIRECTORY|unix.O_NOFOLLOW|unix.O_CLOEXEC, 0)
	if err != nil {
		return errors.New("that app has already closed")
	}
	defer unix.Close(fd)
	held := fmt.Sprintf("/proc/self/fd/%d/.", fd)
	current, err := scopeToken(path)
	if err != nil || token == "" || current != token {
		return errors.New("app scope changed; select it again")
	}
	pids := scopePIDs(held)
	a := App{Unit: unit}
	for _, pid := range pids {
		if p, ok := s.process(pid); ok {
			a.Processes = append(a.Processes, p)
		}
	}
	if why := s.readOnly(a); why != "" {
		return errors.New(why)
	}
	if len(a.Processes) != len(pids) {
		return errors.New("scope ownership changed")
	}
	fds := []int{}
	defer func() {
		for _, fd := range fds {
			unix.Close(fd)
		}
	}()
	for _, p := range a.Processes {
		pfd, err := unix.PidfdOpen(p.PID, 0)
		if err != nil {
			return errors.New("process changed or pidfds are unavailable")
		}
		fds = append(fds, pfd)
		q, ok := s.process(p.PID)
		if !ok || q.Start != p.Start {
			return errors.New("process changed; select the app again")
		}
		// The process must still belong to the exact selected scope.
		found := false
		for _, pid := range scopePIDs(held) {
			found = found || pid == p.PID
		}
		if !found {
			return errors.New("process left the app scope")
		}
	}
	if current, err := scopeToken(path); err != nil || current != token {
		return errors.New("app scope changed")
	}
	if force {
		k, err := unix.Openat(fd, "cgroup.kill", unix.O_WRONLY|unix.O_NOFOLLOW|unix.O_CLOEXEC, 0)
		if err != nil {
			return err
		}
		defer unix.Close(k)
		_, err = unix.Write(k, []byte("1"))
		return err
	}
	var failed error
	for _, pfd := range fds {
		if err := unix.PidfdSendSignal(pfd, unix.SIGTERM, nil, 0); err != nil && err != unix.ESRCH {
			failed = err
		}
	}
	return failed
}
