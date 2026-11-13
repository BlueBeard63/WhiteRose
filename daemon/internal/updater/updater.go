// Package updater is the Updates page's side of whiterosed: checking for a
// release, downloading and verifying it (as you, into your cache), installing
// it beside the running version (as root, through the update-root helper and
// a password prompt), and the choices the page and the "couldn't start"
// prompt offer (apply, roll back, try again, stay, automatic rollback), which
// are the session launcher's state (internal/launch).
//
// Checks run on a schedule (daily by default); a newer release is downloaded
// straight away unless the connection is metered. Installing always waits for
// you: it needs an administrator's password.
package updater

import (
	"context"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"slices"
	"strings"
	"sync"
	"time"

	"whiterose/daemon/internal/launch"
	"whiterose/daemon/internal/update"
)

// Saved is what's remembered between runs ($XDG_STATE_HOME/whiterose/updates.json).
type Saved struct {
	Checked int64           `json:"checked"`
	Release *update.Release `json:"release,omitempty"`
	// Every published release (for the Releases tab), newest first.
	Releases   []update.Release `json:"releases,omitempty"`
	Downloaded string           `json:"downloaded,omitempty"` // the version in Dir(), verified
	Steps      []update.Step    `json:"steps,omitempty"`
	Error      string           `json:"error,omitempty"`
}

type Manager struct {
	Src     update.Source
	Public  string // the release key ("" when updates aren't set up)
	Kind    string // "deb" or "rpm" ("" when the system isn't supported)
	Root    string // installed versions (/usr/lib/whiterose)
	State   string // $XDG_STATE_HOME
	Cache   string // $XDG_CACHE_HOME
	Running string // this session's version ("" in a build being worked on)
	// Started by the session launcher (so a restart starts it again).
	Launcher bool

	// Install runs the root half: install the packages in dir, keep `keep`
	// (the running version) and `chosen` (a release picked under Releases,
	// or ""), remove older ones. Progress lines go to `line`.
	Install func(ctx context.Context, dir, keep, chosen string, line func(string)) error
	// Restart asks the compositor to restart the session.
	Restart func() error
	Metered func() bool
	// Pre: pre-releases (alpha, beta) are offered too (update.prerelease).
	Pre  func() bool
	Emit func(any)
	Now  func() time.Time

	mu       sync.Mutex
	saved    Saved
	loaded   bool
	busy     string // "", "checking", "downloading", "installing"
	done     int64
	total    int64
	lastEmit time.Time
}

func (m *Manager) now() time.Time {
	if m.Now != nil {
		return m.Now()
	}
	return time.Now()
}

func (m *Manager) pre() bool { return m.Pre != nil && m.Pre() }

func (m *Manager) savedFile() string  { return filepath.Join(m.State, "whiterose", "updates.json") }
func (m *Manager) launchFile() string { return launch.StateFile(m.State) }

// Dir is where a version's download goes.
func (m *Manager) Dir(version string) string {
	return filepath.Join(m.Cache, "whiterose", "updates", version)
}

func (m *Manager) load() {
	if m.loaded {
		return
	}
	m.loaded = true
	if b, err := os.ReadFile(m.savedFile()); err == nil {
		_ = json.Unmarshal(b, &m.saved)
	}
}

func (m *Manager) save() {
	b, _ := json.Marshal(m.saved)
	_ = os.MkdirAll(filepath.Dir(m.savedFile()), 0o700)
	tmp := m.savedFile() + ".tmp"
	if os.WriteFile(tmp, b, 0o600) == nil {
		_ = os.Rename(tmp, m.savedFile())
	}
}

// ---- what the page shows -----------------------------------------------------------------

type Step struct {
	Name   string `json:"name"`
	Done   bool   `json:"done"`
	Active bool   `json:"active"` // the one being waited for
	Detail string `json:"detail"`
}

type ReleaseView struct {
	Version   string   `json:"version"`
	Changes   []string `json:"changes"`
	Size      string   `json:"size"`
	Published int64    `json:"published"`
	// available | downloading | downloaded | installing | ready
	State   string `json:"state"`
	Pending bool   `json:"pending"` // applies at the next session start
	Steps   []Step `json:"steps"`
}

type Failure struct {
	Version  string `json:"version"`
	Reason   string `json:"reason"`
	Attempts int    `json:"attempts"`
	Tries    int    `json:"tries"`
	Log      string `json:"log"`
	BackOn   string `json:"back_on"`
}

type View struct {
	Running   string       `json:"running"`
	Supported bool         `json:"supported"` // this is an installed WhiteRose that can update
	Why       string       `json:"why"`       // why not
	Checked   int64        `json:"checked"`
	Busy      string       `json:"busy"`
	Progress  [2]int64     `json:"progress"` // downloaded, of
	Error     string       `json:"error"`
	Release   *ReleaseView `json:"release"`
	Previous  string       `json:"previous"`
	RolledTo  string       `json:"rolled_to"` // rolled back: this from the next start
	Waiting   string       `json:"waiting"`   // a newer version installed, started (and checked) at the next start
	Rollback  bool         `json:"auto_rollback"`
	Snapshot  int64        `json:"snapshot"`
	Launcher  bool         `json:"launcher"`
	Failure   *Failure     `json:"failure"`
}

func (m *Manager) why() string {
	switch {
	case m.Running == "":
		return "This is a build being worked on: updates are for installed WhiteRose."
	case m.Kind == "":
		return "Updates need Ubuntu or Fedora."
	case m.Public == "" || m.Src.Repo == "":
		return "Updates aren't set up for this build (no release key or source)."
	}
	return ""
}

// Status is the page's view.
func (m *Manager) Status() View {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.status()
}

func (m *Manager) status() View {
	m.load()
	st := launch.Load(m.launchFile())
	installed := update.Installed(m.Root)
	v := View{
		Running: m.Running, Why: m.why(), Checked: m.saved.Checked, Busy: m.busy,
		Progress: [2]int64{m.done, m.total}, Error: m.saved.Error,
		Rollback: !st.NoRollback, Snapshot: st.Snapshot, Launcher: m.Launcher,
	}
	v.Supported = v.Why == ""
	// The one to go back to: the other version kept on disk.
	for _, k := range update.Keep(installed, m.Running) {
		if k != m.Running && m.Running != "" && update.Newer(m.Running, k) {
			v.Previous = k
		}
	}
	if st.Current != "" && st.Current != m.Running && slices.Contains(installed, st.Current) {
		v.RolledTo = st.Current
	}
	// Installed but not running yet (by the Updates page, apt / dnf, or a
	// release's install.sh): what the launcher would start next.
	if plan, _, err := launch.Decide(st, installed, update.Newer); err == nil && v.RolledTo == "" &&
		m.Running != "" && update.Newer(plan.Version, m.Running) {
		v.Waiting = plan.Version
	}
	if st.Failed != "" {
		v.Failure = &Failure{Version: st.Failed, Reason: st.Reason, Attempts: st.Attempts, Tries: launch.Tries,
			Log: filepath.Join(m.State, "whiterose", "update-"+st.Failed+".log"), BackOn: m.Running}
	}

	r := m.saved.Release
	if r == nil || m.Running == "" || !update.Newer(r.Version(), m.Running) {
		return v
	}
	ver := r.Version()
	rv := &ReleaseView{Version: ver, Changes: r.Changes(), Published: r.Published.Unix()}
	var size int64
	for _, p := range r.Packages(m.Kind) {
		size += p.Size
	}
	rv.Size = update.Size(size)
	isInstalled := slices.Contains(installed, ver)
	downloaded := m.saved.Downloaded == ver
	switch {
	case m.busy == "installing":
		rv.State = "installing"
	case isInstalled:
		rv.State = "ready"
	case m.busy == "downloading":
		rv.State = "downloading"
	case downloaded:
		rv.State = "downloaded"
	default:
		rv.State = "available"
	}
	rv.Pending = isInstalled && st.Pending == ver
	rv.Steps = []Step{
		{Name: "Downloaded", Detail: rv.Size},
		{Name: "Signature", Detail: "Release key " + update.KeyID(m.Public)},
		{Name: "Files verified", Detail: "sha256"},
		{Name: "Installed alongside", Detail: m.Running + " kept on disk"},
		{Name: "Switch on restart", Detail: "Health check · auto rollback"},
	}
	if st.NoRollback {
		rv.Steps[4].Detail = "No health check"
	}
	if downloaded || isInstalled {
		for i, s := range m.saved.Steps {
			if i < 3 {
				rv.Steps[i].Done, rv.Steps[i].Detail = s.Done, s.Detail
			}
		}
	}
	if isInstalled {
		for i := range 3 {
			rv.Steps[i].Done = true
		}
		rv.Steps[3].Done = true
	}
	for i := range rv.Steps {
		if !rv.Steps[i].Done {
			rv.Steps[i].Active = true
			break
		}
	}
	if m.busy == "downloading" && m.total > 0 {
		rv.Steps[0].Detail = update.Size(m.done) + " of " + update.Size(m.total)
	}
	v.Release = rv
	return v
}

// ---- the Releases tab -------------------------------------------------------------------

type ReleaseEntry struct {
	Version   string   `json:"version"`
	Published int64    `json:"published"`
	Pre       bool     `json:"pre"`
	Changes   []string `json:"changes"`
	// running | next (starts at the next login) | installed | available
	State string `json:"state"`
}

// Series is one major.minor ("0.3") and its releases, newest first.
type Series struct {
	Name     string         `json:"name"`
	Releases []ReleaseEntry `json:"releases"`
}

// Releases is every known release grouped by series, newest first.
// Pre-releases only show with the pre-release switch on, or when one is
// the version you're on (or the one starting at the next login).
func (m *Manager) Releases() []Series {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.load()
	installed := update.Installed(m.Root)
	next := ""
	if plan, _, err := launch.Decide(launch.Load(m.launchFile()), installed, update.Newer); err == nil && plan.Version != m.Running {
		next = plan.Version
	}
	pre := m.pre()
	out := []Series{}
	for _, r := range m.saved.Releases {
		v := r.Version()
		e := ReleaseEntry{Version: v, Published: r.Published.Unix(), Pre: r.Pre, Changes: r.Changes(), State: "available"}
		switch {
		case v == m.Running:
			e.State = "running"
		case v == next:
			e.State = "next"
		case slices.Contains(installed, v):
			e.State = "installed"
		}
		if r.Pre && !pre && e.State != "running" && e.State != "next" {
			continue
		}
		name := update.Series(v)
		if n := len(out); n == 0 || out[n-1].Name != name {
			out = append(out, Series{Name: name})
		}
		out[len(out)-1].Releases = append(out[len(out)-1].Releases, e)
	}
	return out
}

func (m *Manager) emit() {
	if m.Emit != nil {
		v := m.status()
		m.Emit(map[string]any{"event": "updates", "status": v})
	}
}

// ---- doing ----------------------------------------------------------------------------------

func (m *Manager) start(what string) error {
	if m.busy != "" {
		return errors.New("already " + m.busy)
	}
	if w := m.why(); w != "" {
		return errors.New(w)
	}
	m.busy = what
	m.emit()
	return nil
}

func (m *Manager) finish(err error) {
	m.busy = ""
	m.done, m.total = 0, 0
	if err != nil {
		m.saved.Error = err.Error()
	} else {
		m.saved.Error = ""
	}
	m.save()
	m.emit()
}

// Due: time for a scheduled check.
func (m *Manager) Due(every time.Duration) bool {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.load()
	return every > 0 && m.why() == "" && m.now().Sub(time.Unix(m.saved.Checked, 0)) >= every
}

// Check asks for the latest release; with `fetch`, a newer one is
// downloaded too (not on a metered connection).
func (m *Manager) Check(ctx context.Context, fetch bool) error {
	m.mu.Lock()
	m.load()
	if err := m.start("checking"); err != nil {
		m.mu.Unlock()
		return err
	}
	m.mu.Unlock()

	list, err := m.Src.List(ctx)
	r, ok := update.Newest(list, m.pre())
	if err == nil && !ok {
		err = errors.New("there are no releases yet")
	}

	m.mu.Lock()
	m.saved.Checked = m.now().Unix()
	if err == nil {
		m.saved.Releases = list
		m.saved.Release = &r
		if m.saved.Downloaded != "" && m.saved.Downloaded != r.Version() {
			// An older download: not needed any more.
			os.RemoveAll(m.Dir(m.saved.Downloaded))
			m.saved.Downloaded, m.saved.Steps = "", nil
		}
	}
	m.finish(err)
	want := err == nil && fetch && update.Newer(r.Version(), m.Running) &&
		m.saved.Downloaded != r.Version() && !slices.Contains(update.Installed(m.Root), r.Version()) &&
		(m.Metered == nil || !m.Metered())
	m.mu.Unlock()
	if err != nil {
		return err
	}
	if want {
		return m.Download(ctx)
	}
	return nil
}

// Download fetches and verifies the latest release (as you, into your cache).
func (m *Manager) Download(ctx context.Context) error {
	m.mu.Lock()
	m.load()
	r := m.saved.Release
	if r == nil || !update.Newer(r.Version(), m.Running) {
		m.mu.Unlock()
		return errors.New("there's no update to download")
	}
	if err := m.start("downloading"); err != nil {
		m.mu.Unlock()
		return err
	}
	rel := *r
	m.mu.Unlock()

	dir := m.Dir(rel.Version())
	os.RemoveAll(dir)
	_, steps, err := m.Src.Fetch(ctx, rel, m.Kind, m.Public, dir, func(done, total int64) {
		m.mu.Lock()
		defer m.mu.Unlock()
		m.done, m.total = done, total
		// A few times a second at most.
		if t := m.now(); t.Sub(m.lastEmit) > 250*time.Millisecond || done == total {
			m.lastEmit = t
			m.emit()
		}
	})

	m.mu.Lock()
	defer m.mu.Unlock()
	m.saved.Steps = steps
	if err == nil {
		m.saved.Downloaded = rel.Version()
	} else {
		os.RemoveAll(dir)
		m.saved.Downloaded = ""
	}
	m.finish(err)
	return err
}

// InstallNow installs the downloaded release beside the running one (the
// root helper asks for an administrator's password).
func (m *Manager) InstallNow(ctx context.Context) error {
	m.mu.Lock()
	m.load()
	ver := m.saved.Downloaded
	if ver == "" || m.saved.Release == nil || m.saved.Release.Version() != ver {
		m.mu.Unlock()
		return errors.New("download the update first")
	}
	if err := m.start("installing"); err != nil {
		m.mu.Unlock()
		return err
	}
	m.mu.Unlock()

	err := m.Install(ctx, m.Dir(ver), m.Running, "", func(string) {})

	m.mu.Lock()
	defer m.mu.Unlock()
	if err == nil {
		// Installed: the download isn't needed any more.
		os.RemoveAll(m.Dir(ver))
		m.saveLaunch(func(s launch.State) launch.State { return launch.Apply(s, ver) })
	}
	m.finish(err)
	return err
}

func (m *Manager) saveLaunch(fn func(launch.State) launch.State) error {
	f := m.launchFile()
	return launch.Save(f, fn(launch.Load(f)))
}

// change applies a launcher-state choice and tells the page.
func (m *Manager) change(fn func(launch.State) launch.State) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	err := m.saveLaunch(fn)
	m.emit()
	return err
}

// Apply: the installed update starts at the next session start.
func (m *Manager) Apply() error {
	m.mu.Lock()
	r := m.saved.Release
	m.mu.Unlock()
	if r == nil || !slices.Contains(update.Installed(m.Root), r.Version()) {
		return errors.New("install the update first")
	}
	return m.change(func(s launch.State) launch.State {
		if s.Current == "" {
			s.Current = m.Running
		}
		return launch.Apply(s, r.Version())
	})
}

// Rollback: the previous version from the next session start.
func (m *Manager) Rollback() error {
	prev := m.Status().Previous
	if prev == "" {
		return errors.New("there's no previous version to go back to")
	}
	return m.change(func(s launch.State) launch.State {
		s.Current = m.Running
		return launch.Rollback(s, prev)
	})
}

// InstallVersion: a release picked under Settings › Updates › Releases,
// newer or older than this one. Not installed yet: downloaded and checked
// as an update is, then installed beside the others; one older than what's
// installed brings only its core package (whiterose-core-<version>), so
// the whiterose package (the session and commands, shared by every
// version) is never downgraded. Then it's the one started at the next
// login: a newer one with the health check (Apply); an older one like
// Roll back (the newer one isn't tried again until a newer release).
func (m *Manager) InstallVersion(ctx context.Context, version string) error {
	installed := update.Installed(m.Root)
	if !slices.Contains(installed, version) {
		m.mu.Lock()
		m.load()
		var rel *update.Release
		for i := range m.saved.Releases {
			if m.saved.Releases[i].Version() == version {
				rel = &m.saved.Releases[i]
				break
			}
		}
		if rel == nil {
			m.mu.Unlock()
			return errors.New("that release isn't in the list: check for updates, then try again")
		}
		if err := m.start("downloading"); err != nil {
			m.mu.Unlock()
			return err
		}
		r := *rel
		m.mu.Unlock()

		dir := m.Dir(version)
		os.RemoveAll(dir)
		_, _, err := m.Src.Fetch(ctx, r, m.Kind, m.Public, dir, func(done, total int64) {
			m.mu.Lock()
			defer m.mu.Unlock()
			m.done, m.total = done, total
			if t := m.now(); t.Sub(m.lastEmit) > 250*time.Millisecond || done == total {
				m.lastEmit = t
				m.emit()
			}
		})
		if err == nil && len(installed) > 0 && update.Newer(installed[len(installed)-1], version) {
			err = coreOnly(dir, version)
		}
		m.mu.Lock()
		if err != nil {
			os.RemoveAll(dir)
			m.finish(err)
			m.mu.Unlock()
			return err
		}
		m.busy = "installing"
		m.emit()
		m.mu.Unlock()

		err = m.Install(ctx, dir, m.Running, version, func(string) {})
		os.RemoveAll(dir)
		m.mu.Lock()
		m.finish(err)
		m.mu.Unlock()
		if err != nil {
			return err
		}
	}
	installed = update.Installed(m.Root)
	if !slices.Contains(installed, version) {
		return errors.New("it didn't install")
	}
	newest := installed[len(installed)-1]
	return m.change(func(s launch.State) launch.State {
		if s.Current == "" {
			s.Current = m.Running
		}
		if update.Newer(version, s.Current) {
			s = launch.Apply(s, version)
		} else {
			s = launch.Rollback(s, version)
		}
		// Not the newest: the launcher would otherwise start the newest
		// (it moves on to newer installs by itself) until a newer release.
		if version != newest {
			s.Skip = newest
		}
		return s
	})
}

// coreOnly leaves only `version`'s core package in dir (an older release:
// the whiterose package installed stays the newer one).
func coreOnly(dir, version string) error {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return err
	}
	core := false
	for _, e := range entries {
		n := e.Name()
		switch {
		case n == "SHA256SUMS" || n == "SHA256SUMS.sig":
		case strings.HasPrefix(n, update.CorePackage(version)+"_") || strings.HasPrefix(n, update.CorePackage(version)+"-"):
			core = true
		default:
			if err := os.Remove(filepath.Join(dir, n)); err != nil {
				return err
			}
		}
	}
	if !core {
		return errors.New("the release has no core package for this system")
	}
	return nil
}

// UndoRollback: stay on the running version after all.
func (m *Manager) UndoRollback() error {
	return m.change(func(s launch.State) launch.State {
		s.Skip, s.Current = "", m.Running
		return s
	})
}

func (m *Manager) SetAutoRollback(on bool) error {
	return m.change(func(s launch.State) launch.State { s.NoRollback = !on; return s })
}

// TryAgain (the prompt): the failed version gets two more tries, starting now.
func (m *Manager) TryAgain() error {
	if err := m.change(launch.TryAgain); err != nil {
		return err
	}
	return m.RestartSession()
}

// Stay (the prompt): keep this version; the prompt goes away.
func (m *Manager) Stay() error { return m.change(launch.Stay) }

// RestartSession restarts the session (apps close), when the launcher will
// start it again.
func (m *Manager) RestartSession() error {
	if !m.Launcher || m.Restart == nil {
		return errors.New("this session can't restart itself: log out and in again")
	}
	return m.Restart()
}
