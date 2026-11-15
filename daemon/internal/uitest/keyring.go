package uitest

import (
	"fmt"
	"os"
	"os/exec"
	"strings"
	"syscall"

	"github.com/godbus/dbus/v5"
)

const loginKeyring = dbus.ObjectPath("/org/freedesktop/secrets/collection/login")

// LockedKeyring starts a GNOME Keyring on the session's bus whose login
// keyring has `password` and is locked, as after a fingerprint login. It
// runs sandboxed (bwrap, with an empty /run/user/<uid>), so it can never
// find or reach the person's own keyring. Skips the test without bwrap or
// GNOME Keyring.
func (s *Session) LockedKeyring(password string) {
	s.t.Helper()
	for _, bin := range []string{"bwrap", "gnome-keyring-daemon"} {
		if _, err := exec.LookPath(bin); err != nil {
			s.t.Skipf("needs %s", bin)
		}
	}
	run := fmt.Sprintf("/run/user/%d", os.Getuid())
	cmd := exec.Command("bwrap", "--dev-bind", "/", "/", "--tmpfs", run, "--die-with-parent",
		"gnome-keyring-daemon", "--unlock", "--foreground", "--components=secrets")
	cmd.Env = []string{
		"PATH=" + os.Getenv("PATH"),
		"HOME=" + s.Home,
		"XDG_RUNTIME_DIR=" + run,
		"XDG_DATA_HOME=" + s.Home + "/.local/share",
		"DBUS_SESSION_BUS_ADDRESS=" + s.busAddr,
	}
	cmd.Stdin = strings.NewReader(password)
	cmd.Stdout, cmd.Stderr = s.log, s.log
	// Its own process group, ended whole: bwrap and the keyring inside it,
	// so nothing keeps the output pipe (or the session's bus) open.
	cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
	if err := cmd.Start(); err != nil {
		s.t.Fatalf("gnome-keyring-daemon: %v", err)
	}
	s.t.Cleanup(func() { _ = syscall.Kill(-cmd.Process.Pid, syscall.SIGKILL); _ = cmd.Wait() })

	conn := s.keyringBus()
	if !waitUntil(Patience, func() bool {
		_, err := s.keyringLocked(conn)
		return err == nil
	}) {
		s.t.Fatal("the keyring didn't start")
	}
	var unlocked []dbus.ObjectPath
	var prompt dbus.ObjectPath
	if err := conn.Object("org.freedesktop.secrets", "/org/freedesktop/secrets").
		Call("org.freedesktop.Secret.Service.Lock", 0, []dbus.ObjectPath{loginKeyring}).Store(&unlocked, &prompt); err != nil {
		s.t.Fatalf("locking the keyring: %v", err)
	}
}

// KeyringLocked: the keyring LockedKeyring started is locked.
func (s *Session) KeyringLocked() bool {
	s.t.Helper()
	locked, err := s.keyringLocked(s.keyringBus())
	if err != nil {
		s.t.Fatalf("the keyring: %v", err)
	}
	return locked
}

// WaitKeyring waits until the keyring is locked (or open).
func (s *Session) WaitKeyring(locked bool) {
	s.t.Helper()
	if !waitUntil(Patience, func() bool { l, err := s.keyringLocked(s.keyringBus()); return err == nil && l == locked }) {
		s.t.Fatalf("the keyring isn't %s", map[bool]string{true: "locked", false: "open"}[locked])
	}
}

func (s *Session) keyringBus() *dbus.Conn {
	s.t.Helper()
	conn, err := dbus.Connect(s.busAddr)
	if err != nil {
		s.t.Fatalf("the session's bus: %v", err)
	}
	s.t.Cleanup(func() { conn.Close() })
	return conn
}

func (s *Session) keyringLocked(conn *dbus.Conn) (bool, error) {
	v, err := conn.Object("org.freedesktop.secrets", loginKeyring).GetProperty("org.freedesktop.Secret.Collection.Locked")
	if err != nil {
		return false, err
	}
	b, _ := v.Value().(bool)
	return b, nil
}
