package main

import (
	"bufio"
	"context"
	"errors"
	"fmt"
	"io"
	"net"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
	"time"

	"github.com/godbus/dbus/v5"

	"whiterose/daemon/internal/apps"
	"whiterose/daemon/internal/server"
	"whiterose/daemon/internal/store"
	"whiterose/daemon/internal/update"
	"whiterose/daemon/internal/updater"
)

// Installed by the packages: the release key and where releases come from.
const (
	installedKeyFile = "/usr/share/whiterose/release.pub"
	releaseRepoFile  = "/usr/share/whiterose/release-repo"
)

func packageKind() string {
	switch d, _ := apps.DetectDistro("/etc/os-release"); d {
	case apps.Ubuntu:
		return "deb"
	case apps.Fedora:
		return "rpm"
	}
	return ""
}

var versionRe = regexp.MustCompile(`^[0-9][0-9A-Za-z.~-]{0,39}$`)

// cmdUpdateRoot installs a downloaded update, run through pkexec:
//
//	whiterosed update-root DIR KEEP
//
// It copies the download into a folder of its own, checks it again (the
// signature by the installed release key, every package against the signed
// checksums: nothing in DIR is trusted), installs the packages with apt or
// dnf beside the versions already there, then uninstalls versions older
// than the two kept (the new one, and KEEP: the one running).
func cmdUpdateRoot(args []string, stdout, stderr io.Writer) int {
	fail := func(f string, a ...any) int {
		fmt.Fprintf(stderr, "whiterosed update-root: "+f+"\n", a...)
		return 1
	}
	if os.Geteuid() != 0 {
		return fail("run through pkexec (it installs system packages)")
	}
	if len(args) != 2 || !versionRe.MatchString(args[1]) {
		fmt.Fprintln(stderr, "usage: whiterosed update-root DIR KEEP_VERSION")
		return 2
	}
	src, keep := args[0], args[1]
	kind := packageKind()
	if kind == "" {
		return fail("this distribution isn't supported (Ubuntu or Fedora)")
	}
	pub, err := os.ReadFile(installedKeyFile)
	if err != nil {
		return fail("no release key: %v", err)
	}

	// A copy only root can change, readable by apt's sandbox user.
	work, err := os.MkdirTemp("/var/tmp", "whiterose-update-")
	if err != nil {
		return fail("%v", err)
	}
	defer os.RemoveAll(work)
	_ = os.Chmod(work, 0o755)
	entries, err := os.ReadDir(src)
	if err != nil {
		return fail("%v", err)
	}
	for _, e := range entries {
		n := e.Name()
		if n != "SHA256SUMS" && n != "SHA256SUMS.sig" && !update.PackageName(n, kind) {
			continue
		}
		// Plain files only (not links to something else).
		if !e.Type().IsRegular() {
			return fail("%s isn't a plain file", n)
		}
		if err := copyFile(filepath.Join(src, n), filepath.Join(work, n)); err != nil {
			return fail("%v", err)
		}
	}
	pkgs, err := update.VerifyDir(work, kind, strings.TrimSpace(string(pub)))
	if err != nil {
		return fail("%v", err)
	}
	fmt.Fprintln(stdout, "verified", len(pkgs), "packages")

	run := func(name string, args ...string) error {
		cmd := exec.Command(name, args...)
		cmd.Env = append(os.Environ(), "DEBIAN_FRONTEND=noninteractive")
		cmd.Stdout, cmd.Stderr = stdout, stderr
		return cmd.Run()
	}
	if kind == "deb" {
		err = run("apt-get", append([]string{"install", "-y", "--no-install-recommends"}, pkgs...)...)
	} else {
		err = run("dnf", append([]string{"install", "-y"}, pkgs...)...)
	}
	if err != nil {
		return fail("installing: %v", err)
	}

	// Never more than two versions on disk.
	for _, v := range update.Remove(update.Installed(update.Root), keep) {
		pkg := update.CorePackage(v)
		fmt.Fprintln(stdout, "removing", pkg)
		if kind == "deb" {
			err = run("apt-get", "remove", "-y", pkg)
		} else {
			err = run("dnf", "remove", "-y", pkg)
		}
		if err != nil {
			// Installed fine; an old version left over isn't a failure.
			fmt.Fprintf(stderr, "whiterosed update-root: removing %s: %v\n", pkg, err)
		}
	}
	return 0
}

func copyFile(from, to string) error {
	in, err := os.Open(from)
	if err != nil {
		return err
	}
	defer in.Close()
	out, err := os.OpenFile(to, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0o644)
	if err != nil {
		return err
	}
	if _, err := io.Copy(out, io.LimitReader(in, 1<<30)); err != nil {
		out.Close()
		return err
	}
	return out.Close()
}

// runningVersion is this session's version: what the launcher said, else
// the VERSION file of the folder it runs from ("" for a build being worked on).
func runningVersion() string {
	if v := os.Getenv("WHITEROSE_VERSION"); versionRe.MatchString(v) {
		return v
	}
	if d := os.Getenv("WHITEROSE_DIR"); d != "" {
		if b, err := os.ReadFile(filepath.Join(d, "VERSION")); err == nil {
			if v := strings.TrimSpace(string(b)); versionRe.MatchString(v) {
				return v
			}
		}
	}
	return ""
}

// metered: NetworkManager says the connection costs (or probably does).
func metered() bool {
	conn, err := dbus.ConnectSystemBus()
	if err != nil {
		return false
	}
	defer conn.Close()
	v, err := conn.Object("org.freedesktop.NetworkManager", "/org/freedesktop/NetworkManager").
		GetProperty("org.freedesktop.NetworkManager.Metered")
	if err != nil {
		return false
	}
	m, _ := v.Value().(uint32)
	return m == 1 || m == 3 // yes, guess-yes
}

// restartSession asks the compositor to restart (it exits for the launcher
// to start again).
func restartSession() error {
	sock := os.Getenv("WHITEROSE_SOCKET")
	if sock == "" {
		return errors.New("not running in a WhiteRose session")
	}
	c, err := net.DialTimeout("unix", sock, 2*time.Second)
	if err != nil {
		return err
	}
	defer c.Close()
	_, err = c.Write([]byte(`{"cmd":"restart"}` + "\n"))
	return err
}

func newUpdater(st *store.Store, cache string, broadcast func(any)) *updater.Manager {
	// (Overrides for trying it from a build: a test release key, repository,
	// API address and versions folder.)
	env := func(k, def string) string {
		if v := os.Getenv(k); v != "" {
			return v
		}
		return def
	}
	pub, _ := os.ReadFile(env("WHITEROSE_RELEASE_KEY", installedKeyFile))
	repo, _ := os.ReadFile(releaseRepoFile)
	if r := os.Getenv("WHITEROSE_RELEASE_REPO"); r != "" {
		repo = []byte(r)
	}
	state := os.Getenv("XDG_STATE_HOME")
	if state == "" {
		home, _ := os.UserHomeDir()
		state = filepath.Join(home, ".local", "state")
	}
	return &updater.Manager{
		Src:      update.Source{Repo: strings.TrimSpace(string(repo)), API: os.Getenv("WHITEROSE_UPDATE_API")},
		Public:   strings.TrimSpace(string(pub)),
		Kind:     packageKind(),
		Root:     env("WHITEROSE_ROOT", update.Root),
		State:    state,
		Cache:    filepath.Dir(cache), // (cache is ~/.cache/whiterose)
		Running:  runningVersion(),
		Launcher: os.Getenv("WHITEROSE_LAUNCHER") == "1",
		Metered:  metered,
		Pre: func() bool {
			var on bool
			_ = st.GetInto("update.prerelease", &on)
			return on
		},
		Restart: restartSession,
		Emit:    broadcast,
		Install: func(ctx context.Context, dir, keep string, line func(string)) error {
			self, err := rootSelf()
			if err != nil {
				return err
			}
			var stderr bytesTail
			cmd := exec.CommandContext(ctx, "pkexec", self, "update-root", dir, keep)
			cmd.Stderr = &stderr
			out, err := cmd.StdoutPipe()
			if err != nil {
				return err
			}
			if err := cmd.Start(); err != nil {
				return err
			}
			sc := bufio.NewScanner(out)
			for sc.Scan() {
				line(sc.Text())
			}
			if err := cmd.Wait(); err != nil {
				var exit *exec.ExitError
				if errors.As(err, &exit) && (exit.ExitCode() == 126 || exit.ExitCode() == 127) {
					return errors.New("not allowed (wrong password, or cancelled)")
				}
				return fmt.Errorf("installing failed%s", stderr.suffix())
			}
			return nil
		},
	}
}

// checkInterval: the update.check setting.
func checkInterval(st *store.Store) time.Duration {
	var how string
	_ = st.GetInto("update.check", &how)
	switch how {
	case "weekly":
		return 7 * 24 * time.Hour
	case "manual":
		return 0
	}
	return 24 * time.Hour
}

// scheduleUpdates checks when due (looked at hourly; not in the first
// minutes after login, when everything else is starting).
func scheduleUpdates(m *updater.Manager, st *store.Store) {
	time.Sleep(3 * time.Minute)
	for {
		if m.Due(checkInterval(st)) {
			ctx, cancel := context.WithTimeout(context.Background(), 30*time.Minute)
			_ = m.Check(ctx, true)
			cancel()
		}
		time.Sleep(time.Hour)
	}
}

func updateOps(m *updater.Manager) map[string]func(server.Request) (any, error) {
	status := func(err error) (any, error) {
		if err != nil {
			return nil, err
		}
		return m.Status(), nil
	}
	long := func() (context.Context, context.CancelFunc) {
		return context.WithTimeout(context.Background(), 30*time.Minute)
	}
	return map[string]func(server.Request) (any, error){
		"updates": func(server.Request) (any, error) { return m.Status(), nil },
		"update_check": func(server.Request) (any, error) {
			ctx, cancel := long()
			defer cancel()
			return status(m.Check(ctx, true))
		},
		"update_download": func(server.Request) (any, error) {
			ctx, cancel := long()
			defer cancel()
			return status(m.Download(ctx))
		},
		"update_install": func(server.Request) (any, error) {
			ctx, cancel := long()
			defer cancel()
			return status(m.InstallNow(ctx))
		},
		"update_releases":      func(server.Request) (any, error) { return m.Releases(), nil },
		"update_apply":         func(server.Request) (any, error) { return status(m.Apply()) },
		"update_restart":       func(server.Request) (any, error) { return status(m.RestartSession()) },
		"update_rollback":      func(server.Request) (any, error) { return status(m.Rollback()) },
		"update_undo_rollback": func(server.Request) (any, error) { return status(m.UndoRollback()) },
		"update_auto_rollback": func(r server.Request) (any, error) { return status(m.SetAutoRollback(r.On)) },
		"update_try_again":     func(server.Request) (any, error) { return status(m.TryAgain()) },
		"update_stay":          func(server.Request) (any, error) { return status(m.Stay()) },
	}
}
