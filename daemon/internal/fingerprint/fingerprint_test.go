package fingerprint

import (
	"bufio"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/godbus/dbus/v5"
	"github.com/godbus/dbus/v5/prop"
)

// A fake fprintd on a private bus.
type fakeDev struct {
	mu      sync.Mutex
	conn    *dbus.Conn
	fingers []string
	claimed bool
	deny    bool          // Claim: permission denied (password cancelled)
	script  []string      // EnrollStatus results to send, one per touch
	hold    bool          // after the script, wait (for cancelling)
	stopped chan struct{} // EnrollStop called
}

// set changes the fake under its lock; isClaimed reads it.
func (d *fakeDev) set(fn func())   { d.mu.Lock(); fn(); d.mu.Unlock() }
func (d *fakeDev) isClaimed() bool { d.mu.Lock(); defer d.mu.Unlock(); return d.claimed }

const devPath = dbus.ObjectPath("/net/reactivated/Fprint/Device/0")

func dbusErr(name string) *dbus.Error {
	return dbus.NewError("net.reactivated.Fprint.Error."+name, nil)
}

func (d *fakeDev) GetDefaultDevice() (dbus.ObjectPath, *dbus.Error) { return devPath, nil }

func (d *fakeDev) Claim(user string) *dbus.Error {
	d.mu.Lock()
	defer d.mu.Unlock()
	if d.deny {
		return dbusErr("PermissionDenied")
	}
	if d.claimed {
		return dbusErr("AlreadyInUse")
	}
	d.claimed = true
	return nil
}

func (d *fakeDev) Release() *dbus.Error {
	d.mu.Lock()
	d.claimed = false
	d.mu.Unlock()
	return nil
}

func (d *fakeDev) ListEnrolledFingers(user string) ([]string, *dbus.Error) {
	d.mu.Lock()
	defer d.mu.Unlock()
	if len(d.fingers) == 0 {
		return nil, dbusErr("NoEnrolledPrints")
	}
	return d.fingers, nil
}

func (d *fakeDev) EnrollStart(finger string) *dbus.Error {
	d.mu.Lock()
	defer d.mu.Unlock()
	if !d.claimed {
		return dbusErr("ClaimDevice")
	}
	script, hold := d.script, d.hold
	go func() {
		for i, r := range script {
			time.Sleep(10 * time.Millisecond)
			done := r == "enroll-completed" || r == "enroll-duplicate" || (i == len(script)-1 && !hold)
			d.conn.Emit(devPath, deviceIfc+".EnrollStatus", r, done)
			if r == "enroll-completed" {
				d.mu.Lock()
				d.fingers = append(d.fingers, finger)
				d.mu.Unlock()
			}
		}
	}()
	return nil
}

func (d *fakeDev) EnrollStop() *dbus.Error {
	if d.stopped != nil {
		select {
		case d.stopped <- struct{}{}:
		default:
		}
	}
	return nil
}

func (d *fakeDev) DeleteEnrolledFinger(finger string) *dbus.Error {
	d.mu.Lock()
	defer d.mu.Unlock()
	if !d.claimed {
		return dbusErr("ClaimDevice")
	}
	d.fingers = slices.DeleteFunc(d.fingers, func(f string) bool { return f == finger })
	return nil
}

func fakeFprintd(t *testing.T) (*fakeDev, *Service) {
	t.Helper()
	if _, err := exec.LookPath("dbus-daemon"); err != nil {
		t.Skip("no dbus-daemon")
	}
	conf := filepath.Join(t.TempDir(), "bus.conf")
	os.WriteFile(conf, []byte(`<busconfig><type>session</type><listen>unix:tmpdir=`+t.TempDir()+`</listen>
<policy context="default"><allow send_destination="*" eavesdrop="true"/><allow eavesdrop="true"/><allow own="*"/></policy></busconfig>`), 0o600)
	bus := exec.Command("dbus-daemon", "--config-file="+conf, "--nofork", "--print-address=1")
	out, _ := bus.StdoutPipe()
	if err := bus.Start(); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { bus.Process.Kill(); bus.Wait() })
	addr, err := bufio.NewReader(out).ReadString('\n')
	if err != nil {
		t.Fatal(err)
	}
	addr = strings.TrimSpace(addr)

	srv, err := dbus.Connect(addr)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { srv.Close() })
	d := &fakeDev{conn: srv}
	srv.Export(d, managerObj, managerIfc)
	srv.Export(d, devPath, deviceIfc)
	if _, err := prop.Export(srv, devPath, prop.Map{deviceIfc: {
		"name":              {Value: "Test Reader"},
		"num-enroll-stages": {Value: int32(3)},
		"scan-type":         {Value: "press"},
	}}); err != nil {
		t.Fatal(err)
	}
	if r, err := srv.RequestName(busName, dbus.NameFlagDoNotQueue); err != nil || r != dbus.RequestNameReplyPrimaryOwner {
		t.Fatal("name", r, err)
	}

	cli, err := dbus.Connect(addr)
	if err != nil {
		t.Fatal(err)
	}
	s := NewOn(cli)
	t.Cleanup(s.Close)
	return d, s
}

func TestStatusAndDelete(t *testing.T) {
	d, s := fakeFprintd(t)
	st, err := s.Status()
	if err != nil || st.Reader != "Test Reader" || st.Stages != 3 || st.Scan != "press" || len(st.Fingers) != 0 {
		t.Fatalf("%+v %v", st, err)
	}
	d.set(func() { d.fingers = []string{"left-thumb", "right-index-finger"} })
	st, _ = s.Status()
	if !slices.Equal(st.Fingers, []string{"right-index-finger", "left-thumb"}) {
		t.Errorf("not in the page's order: %v", st.Fingers)
	}
	if err := s.Delete("left-thumb"); err != nil {
		t.Fatal(err)
	}
	if st, _ = s.Status(); !slices.Equal(st.Fingers, []string{"right-index-finger"}) || d.isClaimed() {
		t.Errorf("after delete: %v", st.Fingers)
	}
	if s.Delete("tentacle") == nil {
		t.Error("an unknown finger accepted")
	}
	d.set(func() { d.deny = true })
	if err := s.Delete("right-index-finger"); err == nil || !strings.Contains(err.Error(), "not allowed") {
		t.Errorf("denied: %v", err)
	}
}

func TestEnroll(t *testing.T) {
	d, s := fakeFprintd(t)
	d.set(func() {
		d.script = []string{"enroll-stage-passed", "enroll-finger-not-centered", "enroll-stage-passed", "enroll-completed"}
	})
	var got []Progress
	err := s.Enroll("left-index-finger", func(p Progress) { got = append(got, p) })
	if err != nil {
		t.Fatal(err)
	}
	states := []string{}
	for _, p := range got {
		states = append(states, p.State)
	}
	if strings.Join(states, ",") != "stage,stage,retry,stage,done" {
		t.Fatalf("%v", got)
	}
	if got[1].Stage != 1 || got[3].Stage != 2 || got[4].Stage != 3 || got[4].Stages != 3 || !strings.Contains(got[2].Message, "middle") {
		t.Errorf("%+v", got)
	}
	if st, _ := s.Status(); !slices.Equal(st.Fingers, []string{"left-index-finger"}) || d.isClaimed() {
		t.Errorf("%v", st.Fingers)
	}

	// Already there.
	d.set(func() { d.script = []string{"enroll-duplicate"} })
	if err := s.Enroll("right-thumb", func(Progress) {}); err == nil || !strings.Contains(err.Error(), "already") {
		t.Errorf("duplicate: %v", err)
	}
	// Password cancelled.
	d.set(func() { d.deny = true })
	if err := s.Enroll("right-thumb", func(Progress) {}); err == nil || !strings.Contains(err.Error(), "not allowed") {
		t.Errorf("denied: %v", err)
	}
}

func TestEnrollCancelAndIdle(t *testing.T) {
	d, s := fakeFprintd(t)
	d.set(func() {
		d.script = []string{"enroll-stage-passed"}
		d.hold = true
		d.stopped = make(chan struct{}, 1)
	})
	done := make(chan error)
	var mu sync.Mutex
	var last Progress
	go func() {
		done <- s.Enroll("right-thumb", func(p Progress) { mu.Lock(); last = p; mu.Unlock() })
	}()
	time.Sleep(100 * time.Millisecond)
	if s.Enroll("left-thumb", func(Progress) {}) == nil {
		t.Error("two enrolments at once")
	}
	s.Cancel()
	select {
	case err := <-done:
		mu.Lock()
		defer mu.Unlock()
		if err != nil || last.State != "cancelled" {
			t.Errorf("%v %+v", err, last)
		}
	case <-time.After(3 * time.Second):
		t.Fatal("cancel didn't stop it")
	}
	select {
	case <-d.stopped:
	default:
		t.Error("EnrollStop wasn't called")
	}

	// Nobody touches the reader.
	s.Idle = 150 * time.Millisecond
	d.set(func() { d.script = nil })
	if err := s.Enroll("right-thumb", func(Progress) {}); err == nil || !strings.Contains(err.Error(), "wasn't touched") {
		t.Errorf("idle: %v", err)
	}
	if d.isClaimed() {
		t.Error("reader left claimed")
	}
}

func TestLogin(t *testing.T) {
	root := t.TempDir()
	have = func(string) bool { return true }
	t.Cleanup(func() { have = func(n string) bool { _, err := exec.LookPath(n); return err == nil } })
	write := func(p, s string) {
		os.MkdirAll(filepath.Dir(filepath.Join(root, p)), 0o755)
		os.WriteFile(filepath.Join(root, p), []byte(s), 0o644)
	}
	if l := LoginStatus(root, "ubuntu"); l.Supported || l.Enabled {
		t.Errorf("nothing there: %+v", l)
	}
	write("usr/share/pam-configs/fprintd", "Name: Fingerprint authentication\n")
	write("etc/pam.d/common-auth", "auth [success=2 default=ignore] pam_unix.so nullok\n# pam_fprintd.so (a comment)\n")
	if l := LoginStatus(root, "ubuntu"); !l.Supported || l.Enabled {
		t.Errorf("off: %+v", l)
	}
	write("etc/pam.d/common-auth", "auth [success=3 default=ignore] pam_fprintd.so max-tries=1 timeout=10\nauth pam_unix.so\n")
	if l := LoginStatus(root, "ubuntu"); !l.Enabled {
		t.Errorf("on: %+v", l)
	}
	write("etc/authselect/authselect.conf", "sssd\nwith-fingerprint\n")
	write("etc/pam.d/system-auth", "auth sufficient pam_fprintd.so\n")
	if l := LoginStatus(root, "fedora"); !l.Supported || !l.Enabled {
		t.Errorf("fedora: %+v", l)
	}
	c, _ := LoginCommands("ubuntu", false)
	if strings.Join(c[0], " ") != "pam-auth-update --remove fprintd" {
		t.Errorf("%v", c)
	}
	c, _ = LoginCommands("fedora", true)
	if strings.Join(c[0], " ") != "authselect enable-feature with-fingerprint" {
		t.Errorf("%v", c)
	}
	if _, err := LoginCommands("arch", true); err == nil {
		t.Error("unsupported distro")
	}
}
