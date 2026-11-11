// Package uitest runs click-and-check tests against the real WhiteRose: a
// nested compositor in a session of its own (its own HOME, XDG folders,
// D-Bus, accessibility bus and display), apps started in it, and things in
// them found by name through the accessibility tree (AT-SPI), clicked with
// the real pointer and typed into with the real keyboard, then checked by
// reading back what they show.
//
//	s := uitest.Start(t, uitest.Options{})
//	files := s.Open("whiterose-files", "blk — Files", dir)
//	files.Click(uitest.Named("Rename"))
//	files.Type("IMG_")
//	files.WaitFor(uitest.Named("IMG_0412.JPG").Described("→ lab-day 0412.jpg"))
//
// Nothing in it touches the person's own session: a leftover daemon that
// would let it share theirs makes it refuse to start.
package uitest

import (
	"bufio"
	"encoding/json"
	"fmt"
	"net"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
	"sync"
	"syscall"
	"testing"
	"time"
)

// Options for a session.
type Options struct {
	// The shell (bar, island) too; most app tests don't need it.
	Shell bool
	// Settings it starts with (key: value, as Settings would store them).
	Settings map[string]any
	// Start as on a new install: the first-run app setup is offered
	// (Settings opens on Apps). Otherwise it counts as done.
	FirstRun bool
}

// Session is one isolated WhiteRose session for a test.
type Session struct {
	t       testing.TB
	Repo    string
	Home    string // its HOME (Pictures, Documents… for the test to fill)
	wayland string
	ipcPath string
	busAddr string
	cmd     *exec.Cmd
	log     *logBuffer
	mu      sync.Mutex
	windows []windowInfo
	ipc     net.Conn
	a11y    *a11y
	layers  []layerInfo
	// Counts the layer lists the compositor has sent.
	layersSeen int
}

type layerInfo struct {
	Namespace string `json:"namespace"`
	Output    string `json:"output"`
	Layer     string `json:"layer"`
	Geometry  [4]int `json:"geometry"`
}

type windowInfo struct {
	ID       uint64  `json:"id"`
	AppID    *string `json:"app_id"`
	Title    *string `json:"title"`
	Focused  bool    `json:"focused"`
	Geometry *[4]int `json:"geometry"`
}

// RepoRoot finds the repository (the folder holding project.yaml) above the
// test's working directory.
func RepoRoot() (string, error) {
	d, _ := os.Getwd()
	for {
		if _, err := os.Stat(filepath.Join(d, "project.yaml")); err == nil {
			return d, nil
		}
		up := filepath.Dir(d)
		if up == d {
			return "", fmt.Errorf("no project.yaml above %s", d)
		}
		d = up
	}
}

// binary is a WhiteRose program for the session: $WR_UITEST_<NAME> if set,
// else where a build leaves it in the repository.
func (s *Session) binary(env, rel string) string {
	if p := os.Getenv(env); p != "" {
		return p
	}
	return filepath.Join(s.Repo, rel)
}

// Headless says whether sessions run with no screen (the compositor's
// headless backend, apps drawing in software): when there's no desktop to
// nest in (CI), or WR_UITEST_HEADLESS=1.
func Headless() bool {
	return os.Getenv("WR_UITEST_HEADLESS") == "1" || os.Getenv("WAYLAND_DISPLAY") == ""
}

// Start begins a session (stopped when the test ends): nested in the
// desktop it's run from, or headless (see Headless).
func Start(t testing.TB, o Options) *Session {
	t.Helper()
	if os.Getenv("XDG_RUNTIME_DIR") == "" {
		t.Fatal("XDG_RUNTIME_DIR isn't set (Wayland's sockets live there)")
	}
	repo, err := RepoRoot()
	if err != nil {
		t.Fatal(err)
	}
	if err := refuseSharedDaemon(); err != nil {
		t.Fatal(err)
	}
	s := &Session{t: t, Repo: repo, Home: t.TempDir(), log: &logBuffer{}}
	wm := s.binary("WR_UITEST_WM", "compositor/target/release/whiterose-wm")
	daemon := s.binary("WR_UITEST_DAEMON", "build/whiterosed")
	for _, b := range []string{wm, daemon} {
		if _, err := os.Stat(b); err != nil {
			t.Fatalf("%s isn't built (make ui-test builds what it needs)", b)
		}
	}
	for _, d := range []string{".config", ".local/share", ".cache", ".local/state", "Pictures", "Documents"} {
		os.MkdirAll(filepath.Join(s.Home, d), 0o755)
	}
	seed := map[string]any{}
	if !o.FirstRun {
		seed["apps.setupDone"] = true
	}
	for k, v := range o.Settings {
		seed[k] = v
	}
	s.seed(seed)
	busFile := filepath.Join(s.Home, ".bus")
	args := []string{"--daemon", daemon + " serve"}
	headless := Headless()
	if headless {
		args = append([]string{"--backend", "headless"}, args...)
	}
	if o.Shell {
		if _, err := exec.LookPath("qs"); err != nil {
			t.Fatal("the shell needs QuickShell (qs) on PATH")
		}
		args = append(args, "--shell", "qs -p "+filepath.Join(repo, "shell"))
	}
	// The bus address is written down first, so the test can reach the
	// session's D-Bus (and through it the accessibility bus).
	script := `printf %s "$DBUS_SESSION_BUS_ADDRESS" > "$1"; shift; exec "$@"`
	s.cmd = exec.Command("dbus-run-session", append([]string{"--", "sh", "-c", script, "sh", busFile, wm}, args...)...)
	s.cmd.Dir = s.Home
	s.cmd.Env = append(cleanEnv(),
		"HOME="+s.Home,
		"XDG_CONFIG_HOME="+filepath.Join(s.Home, ".config"),
		"XDG_DATA_HOME="+filepath.Join(s.Home, ".local/share"),
		"XDG_CACHE_HOME="+filepath.Join(s.Home, ".cache"),
		"XDG_STATE_HOME="+filepath.Join(s.Home, ".local/state"),
		// Its accessibility bus is named after the display: never the
		// person's own (that would replace their session's).
		"DISPLAY=:"+strconv.Itoa(freeDisplay()),
		"QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1",
		"QT_ACCESSIBILITY=1",
		"WHITEROSE_DEBUG_INPUT=1",
		// Headless, the session has no desktop to name: the accessibility
		// registry only needs to know it's a Wayland one (else it looks for
		// an X server). Apps get the compositor's own from the compositor.
		"WAYLAND_DISPLAY="+map[bool]string{true: "wayland-uitest-none", false: os.Getenv("WAYLAND_DISPLAY")}[headless],
		// With no GPU (headless), Qt Quick draws in software.
		"QT_QUICK_BACKEND="+map[bool]string{true: "software", false: os.Getenv("QT_QUICK_BACKEND")}[headless],
		"PATH="+filepath.Dir(daemon)+":"+os.Getenv("PATH"),
	)
	s.cmd.Stdout, s.cmd.Stderr = s.log, s.log
	s.cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
	if err := s.cmd.Start(); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(s.stop)
	sock := regexp.MustCompile(`socket="(wayland-[0-9]+)"`)
	if !waitUntil(15*time.Second, func() bool {
		m := sock.FindStringSubmatch(stripANSI(s.log.String()))
		if m == nil {
			return false
		}
		s.wayland = m[1]
		return true
	}) {
		t.Fatalf("the compositor didn't start:\n%s", s.log.String())
	}
	s.ipcPath = filepath.Join(os.Getenv("XDG_RUNTIME_DIR"), "whiterose-"+s.wayland+".sock")
	if !waitUntil(10*time.Second, func() bool {
		b, _ := os.ReadFile(busFile)
		s.busAddr = string(b)
		c, err := net.Dial("unix", s.ipcPath)
		if err != nil || s.busAddr == "" {
			return false
		}
		s.ipc = c
		return true
	}) {
		t.Fatalf("couldn't reach the compositor:\n%s", s.log.String())
	}
	go s.readEvents()
	return s
}

// The person's environment, less what would tie the session to theirs
// (and their desktop: the session sets its own WAYLAND_DISPLAY).
func cleanEnv() []string {
	var out []string
	for _, kv := range os.Environ() {
		k, _, _ := strings.Cut(kv, "=")
		switch {
		case k == "HOME" || k == "DISPLAY" || k == "DBUS_SESSION_BUS_ADDRESS" || k == "AT_SPI_BUS_ADDRESS" || k == "PATH" || k == "QT_QUICK_BACKEND" || k == "WAYLAND_DISPLAY":
		case strings.HasPrefix(k, "XDG_") && k != "XDG_RUNTIME_DIR":
		default:
			out = append(out, kv)
		}
	}
	return out
}

// A display number nothing uses (no X server, no accessibility bus).
func freeDisplay() int {
	run := os.Getenv("XDG_RUNTIME_DIR")
	for n := 97; n < 200; n++ {
		_, x := os.Stat(fmt.Sprintf("/tmp/.X11-unix/X%d", n))
		_, b := os.Stat(filepath.Join(run, "at-spi", fmt.Sprintf("bus_%d", n)))
		if os.IsNotExist(x) && os.IsNotExist(b) {
			return n
		}
	}
	return 199
}

// A whiterosed left behind on a Wayland name no compositor holds would be
// the one the nested compositor talks to (with the person's settings).
func refuseSharedDaemon() error {
	run := os.Getenv("XDG_RUNTIME_DIR")
	procs, _ := filepath.Glob("/proc/[0-9]*/comm")
	for _, c := range procs {
		if b, _ := os.ReadFile(c); strings.TrimSpace(string(b)) != "whiterosed" {
			continue
		}
		env, _ := os.ReadFile(filepath.Join(filepath.Dir(c), "environ"))
		for _, kv := range strings.Split(string(env), "\x00") {
			name, ok := strings.CutPrefix(kv, "WAYLAND_DISPLAY=")
			if !ok || name == "" {
				continue
			}
			f, err := os.OpenFile(filepath.Join(run, name+".lock"), os.O_RDWR, 0)
			if err != nil {
				continue
			}
			free := syscall.Flock(int(f.Fd()), syscall.LOCK_EX|syscall.LOCK_NB) == nil
			f.Close()
			if free {
				return fmt.Errorf("whiterosed %s is left over on %s (nothing holds that name): a test session would use it; stop it first", filepath.Base(filepath.Dir(c)), name)
			}
		}
	}
	return nil
}

func (s *Session) stop() {
	if s.ipc != nil {
		s.ipc.Close()
	}
	if s.a11y != nil {
		s.a11y.conn.Close()
	}
	if s.cmd != nil && s.cmd.Process != nil {
		syscall.Kill(-s.cmd.Process.Pid, syscall.SIGTERM)
		done := make(chan struct{})
		go func() { s.cmd.Wait(); close(done) }()
		select {
		case <-done:
		case <-time.After(5 * time.Second):
			syscall.Kill(-s.cmd.Process.Pid, syscall.SIGKILL)
		}
	}
	if s.t.Failed() {
		s.t.Logf("session log:\n%s", tail(s.log.String(), 60))
	}
}

// ---- the compositor ---------------------------------------------------------------

func (s *Session) readEvents() {
	sc := bufio.NewScanner(s.ipc)
	sc.Buffer(make([]byte, 1<<20), 1<<24)
	for sc.Scan() {
		var e struct {
			Event   string       `json:"event"`
			Windows []windowInfo `json:"windows"`
			Layers  []layerInfo  `json:"layers"`
		}
		if json.Unmarshal(sc.Bytes(), &e) != nil {
			continue
		}
		switch e.Event {
		case "windows":
			s.mu.Lock()
			s.windows = e.Windows
			s.mu.Unlock()
		case "layers":
			s.mu.Lock()
			s.layers = e.Layers
			s.layersSeen++
			s.mu.Unlock()
		}
	}
}

func (s *Session) send(v any) {
	b, _ := json.Marshal(v)
	s.mu.Lock()
	defer s.mu.Unlock()
	if _, err := s.ipc.Write(append(b, '\n')); err != nil {
		s.t.Fatalf("compositor: %v", err)
	}
}

// Spawn runs a command inside the session (as its launcher would).
func (s *Session) Spawn(command string) {
	s.send(map[string]any{"cmd": "spawn", "command": command})
}

// windowAt is the screen position of the window titled `title`.
func (s *Session) window(title string) (windowInfo, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	for _, w := range s.windows {
		if w.Title != nil && *w.Title == title && w.Geometry != nil {
			return w, true
		}
	}
	return windowInfo{}, false
}

// layer is where the layer surface named `namespace` (the bar:
// "whiterose-bar") is now, asked of the compositor.
func (s *Session) layer(namespace string) (layerInfo, bool) {
	s.mu.Lock()
	seen := s.layersSeen
	s.mu.Unlock()
	s.send(map[string]any{"debug_layers": true})
	waitUntil(2*time.Second, func() bool {
		s.mu.Lock()
		defer s.mu.Unlock()
		return s.layersSeen > seen
	})
	s.mu.Lock()
	defer s.mu.Unlock()
	for _, l := range s.layers {
		if l.Namespace == namespace {
			return l, true
		}
	}
	return layerInfo{}, false
}

func (s *Session) layerNames() string {
	s.mu.Lock()
	defer s.mu.Unlock()
	var n []string
	for _, l := range s.layers {
		n = append(n, l.Namespace)
	}
	return strings.Join(n, ", ")
}

// ---- small things -------------------------------------------------------------------

type logBuffer struct {
	mu sync.Mutex
	b  strings.Builder
}

func (l *logBuffer) Write(p []byte) (int, error) {
	l.mu.Lock()
	defer l.mu.Unlock()
	return l.b.Write(p)
}

func (l *logBuffer) String() string {
	l.mu.Lock()
	defer l.mu.Unlock()
	return l.b.String()
}

var ansi = regexp.MustCompile("\x1b\\[[0-9;]*m")

func stripANSI(s string) string { return ansi.ReplaceAllString(s, "") }

func tail(s string, n int) string {
	lines := strings.Split(stripANSI(s), "\n")
	if len(lines) > n {
		lines = lines[len(lines)-n:]
	}
	return strings.Join(lines, "\n")
}

func waitUntil(d time.Duration, ok func() bool) bool {
	end := time.Now().Add(d)
	for time.Now().Before(end) {
		if ok() {
			return true
		}
		time.Sleep(100 * time.Millisecond)
	}
	return ok()
}
