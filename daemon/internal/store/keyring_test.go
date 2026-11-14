package store

import (
	"errors"
	"fmt"
	"os"
	"os/exec"
	"strings"
	"testing"
	"time"

	"github.com/godbus/dbus/v5"
)

// The real GNOME Keyring, locked and unlocked with a password. Never the
// user's own: the test runs again inside a sandbox (bwrap) with its own
// session bus, its own home and an empty /run/user/<uid>, so the real
// keyring and bus aren't there to reach. Skipped without bwrap or GNOME
// Keyring.
func TestGnomeKeyringLockedThenUnlockedWithThePassword(t *testing.T) {
	if os.Getenv("WHITEROSE_KEYRING_SANDBOX") == "" {
		runInKeyringSandbox(t)
		return
	}
	run := fmt.Sprintf("/run/user/%d", os.Getuid())
	if _, err := os.Stat(run + "/bus"); err == nil {
		t.Fatal("not sandboxed: the real session bus is here")
	}
	if !strings.HasPrefix(os.Getenv("DBUS_SESSION_BUS_ADDRESS"), "unix:path=/tmp/") {
		t.Fatalf("not sandboxed: bus %q", os.Getenv("DBUS_SESSION_BUS_ADDRESS"))
	}

	early := make(chan string, 8)
	if err := WatchKeyring(func() {
		select {
		case early <- "x":
		default:
		}
	}); err != nil {
		t.Fatal(err)
	}
	// A keyring whose login password is "pw".
	gk := exec.Command("gnome-keyring-daemon", "--unlock", "--components=secrets")
	gk.Stdin = strings.NewReader("pw")
	if out, err := gk.CombinedOutput(); err != nil {
		t.Fatalf("gnome-keyring-daemon: %v\n%s", err, out)
	}
	var kr Secrets
	var err error
	if !waitFor(func() bool { kr, err = Keyring(); return err == nil }) {
		t.Fatalf("keyring didn't open: %v", err)
	}
	select {
	case <-early:
	case <-time.After(3 * time.Second):
		t.Fatal("the keyring starting wasn't seen")
	}
	if err := kr.Set("cal", "https://cal.example/x"); err != nil {
		t.Fatal(err)
	}

	conn, err := dbus.ConnectSessionBus()
	if err != nil {
		t.Fatal(err)
	}
	defer conn.Close()
	var unlocked []dbus.ObjectPath
	var prompt dbus.ObjectPath
	if err := conn.Object(ssName, ssPath).Call(ssService+".Lock", 0, []dbus.ObjectPath{"/org/freedesktop/secrets/collection/login"}).Store(&unlocked, &prompt); err != nil {
		t.Fatal(err)
	}

	// Locked (as after a fingerprint login): opening says so, and a
	// resolver says the secret is locked rather than missing.
	if _, err := Keyring(); !errors.Is(err, ErrLocked) {
		t.Fatalf("want ErrLocked, got %v", err)
	}
	r := &Resolver{Fallback: memSecrets{}, Open: Keyring}
	if _, err := r.Get("cal"); !errors.Is(err, ErrLocked) {
		t.Fatalf("want ErrLocked, got %v", err)
	}
	if !r.Locked() {
		t.Fatal("not said to be locked")
	}
	changed := make(chan struct{}, 1)
	if err := WatchKeyring(func() {
		select {
		case changed <- struct{}{}:
		default:
		}
	}); err != nil {
		t.Fatal(err)
	}

	if err := UnlockKeyring("not it"); err == nil {
		t.Fatal("unlocked with the wrong password")
	}
	if _, err := Keyring(); !errors.Is(err, ErrLocked) {
		t.Fatalf("a wrong password unlocked it: %v", err)
	}
	if err := UnlockKeyring("pw"); err != nil {
		t.Fatal(err)
	}
	select {
	case <-changed:
	case <-time.After(5 * time.Second):
		t.Fatal("unlocking wasn't seen")
	}
	if !r.Retry() {
		t.Fatalf("keyring didn't open after unlocking: %v", r.Closed())
	}
	if r.Locked() {
		t.Fatal("open, but said to be locked")
	}
	// Locked again while open (by hand): it says so.
	if err := conn.Object(ssName, ssPath).Call(ssService+".Lock", 0, []dbus.ObjectPath{"/org/freedesktop/secrets/collection/login"}).Store(&unlocked, &prompt); err != nil {
		t.Fatal(err)
	}
	if !r.Locked() {
		t.Fatal("locked again, but not said to be")
	}
	if err := UnlockKeyring("pw"); err != nil {
		t.Fatal(err)
	}
	if v, err := r.Get("cal"); err != nil || v != "https://cal.example/x" {
		t.Fatalf("%q %v", v, err)
	}
	// Already unlocked: nothing to do.
	if err := UnlockKeyring("anything"); err != nil {
		t.Fatal(err)
	}
}

// runInKeyringSandbox runs the calling test again inside the sandbox.
func runInKeyringSandbox(t *testing.T) {
	t.Helper()
	for _, bin := range []string{"bwrap", "dbus-run-session", "gnome-keyring-daemon"} {
		if _, err := exec.LookPath(bin); err != nil {
			t.Skipf("needs %s", bin)
		}
	}
	home := t.TempDir()
	run := fmt.Sprintf("/run/user/%d", os.Getuid())
	cmd := exec.Command("bwrap", "--dev-bind", "/", "/", "--tmpfs", run, "--die-with-parent",
		"dbus-run-session", "--", os.Args[0], "-test.run", "^"+t.Name()+"$", "-test.count=1", "-test.v")
	cmd.Env = []string{
		"PATH=" + os.Getenv("PATH"),
		"HOME=" + home,
		"XDG_RUNTIME_DIR=" + run,
		"XDG_DATA_HOME=" + home + "/data",
		"XDG_CONFIG_HOME=" + home + "/config",
		"WHITEROSE_KEYRING_SANDBOX=1",
	}
	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("in the sandbox: %v\n%s", err, out)
	}
	if !strings.Contains(string(out), "--- PASS: "+t.Name()) {
		t.Fatalf("the sandbox didn't run it:\n%s", out)
	}
}

func waitFor(ok func() bool) bool {
	for end := time.Now().Add(5 * time.Second); time.Now().Before(end); time.Sleep(50 * time.Millisecond) {
		if ok() {
			return true
		}
	}
	return false
}

// memSecrets is an empty fallback.
type memSecrets struct{}

func (memSecrets) Name() string               { return "memory" }
func (memSecrets) Get(string) (string, error) { return "", ErrNoSecret }
func (memSecrets) Set(string, string) error   { return nil }
func (memSecrets) Delete(string) error        { return nil }
