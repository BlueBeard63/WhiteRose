package main

import (
	"bufio"
	"context"
	"encoding/binary"
	"io"
	"os/exec"
	"path/filepath"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/godbus/dbus/v5"
	"whiterose/daemon/internal/clocks"
)

type clockNotice struct {
	id      uint32
	actions []string
	hints   map[string]dbus.Variant
	timeout int32
}
type clockNotificationServer struct {
	mu      sync.Mutex
	next    uint32
	notices chan clockNotice
	closed  chan uint32
}

func (s *clockNotificationServer) Notify(app string, replaces uint32, icon, summary, body string, actions []string, hints map[string]dbus.Variant, timeout int32) (uint32, *dbus.Error) {
	s.mu.Lock()
	s.next++
	id := s.next
	s.mu.Unlock()
	s.notices <- clockNotice{id, actions, hints, timeout}
	return id, nil
}
func (s *clockNotificationServer) CloseNotification(id uint32) *dbus.Error {
	s.closed <- id
	return nil
}
func privateClockBus(t *testing.T) string {
	t.Helper()
	bin, err := exec.LookPath("dbus-daemon")
	if err != nil {
		t.Skip("dbus-daemon not installed")
	}
	cmd := exec.Command(bin, "--session", "--nofork", "--print-address=1")
	out, err := cmd.StdoutPipe()
	if err != nil {
		t.Fatal(err)
	}
	if err = cmd.Start(); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { cmd.Process.Kill(); cmd.Wait() })
	sc := bufio.NewScanner(out)
	if !sc.Scan() {
		t.Fatal("private bus didn't start")
	}
	return sc.Text()
}
func waitClockNotice(t *testing.T, ch <-chan clockNotice) clockNotice {
	t.Helper()
	select {
	case n := <-ch:
		return n
	case <-time.After(5 * time.Second):
		t.Fatal("notification wasn't delivered")
		return clockNotice{}
	}
}
func TestClockCriticalNotificationsActionsAndServerReconnect(t *testing.T) {
	address := privateClockBus(t)
	t.Setenv("DBUS_SESSION_BUS_ADDRESS", address)
	server, err := dbus.Connect(address)
	if err != nil {
		t.Fatal(err)
	}
	defer server.Close()
	fake := &clockNotificationServer{notices: make(chan clockNotice, 32), closed: make(chan uint32, 32)}
	const iface = "org.freedesktop.Notifications"
	const path = dbus.ObjectPath("/org/freedesktop/Notifications")
	if err = server.Export(fake, path, iface); err != nil {
		t.Fatal(err)
	}
	if _, err = server.RequestName(iface, dbus.NameFlagDoNotQueue); err != nil {
		t.Fatal(err)
	}
	updates := make(chan clocks.State, 32)
	stamp := clocks.Now()
	var current atomic.Value
	current.Store(stamp)
	s, err := clocks.New(filepath.Join(t.TempDir(), "clocks.json"), time.UTC, func() clocks.Stamp { return current.Load().(clocks.Stamp) }, func(state clocks.State) { updates <- state })
	if err != nil {
		t.Fatal(err)
	}
	done := make(chan struct{})
	finished := make(chan struct{})
	sound := make(chan struct{}, 32)
	go func() {
		defer close(finished)
		runClockNotifications(s, updates, done, io.Discard, func(context.Context, clocks.AlarmOptions, int64) error { sound <- struct{}{}; return nil })
	}()
	defer func() { close(done); <-finished }()
	if _, err = s.Apply(clocks.Command{Action: "timer_add", Name: "Tea", Duration: 1000}); err != nil {
		t.Fatal(err)
	}
	stamp.Boot += 1000
	stamp.Wall += 1000
	current.Store(stamp)
	if err = s.Tick(); err != nil {
		t.Fatal(err)
	}
	n := waitClockNotice(t, fake.notices)
	if n.hints["urgency"].Value() != byte(2) || n.timeout != 0 || len(n.actions) != 4 || n.actions[0] != "restart" {
		t.Fatal(n)
	}
	select {
	case <-sound:
	case <-time.After(time.Second):
		t.Fatal("no chime")
	}
	// A real restart has a new unique bus owner, not only a released name.
	server.Close()
	server, err = dbus.Connect(address)
	if err != nil {
		t.Fatal(err)
	}
	defer server.Close()
	if err = server.Export(fake, path, iface); err != nil {
		t.Fatal(err)
	}
	if _, err = server.RequestName(iface, dbus.NameFlagDoNotQueue); err != nil {
		t.Fatal(err)
	}
	n = waitClockNotice(t, fake.notices)
	if err = server.Emit(path, iface+".ActionInvoked", n.id, "restart"); err != nil {
		t.Fatal(err)
	}
	deadline := time.Now().Add(3 * time.Second)
	for time.Now().Before(deadline) {
		if state := s.Snapshot(); len(state.Alerts) == 0 && state.Timers[0].Running {
			break
		}
		time.Sleep(10 * time.Millisecond)
	}
	if state := s.Snapshot(); len(state.Alerts) != 0 || !state.Timers[0].Running {
		t.Fatal(state)
	}
	select {
	case <-fake.closed:
	case <-time.After(time.Second):
		t.Fatal("app action didn't close notification")
	}
	timer := s.Snapshot().Timers[0]
	if _, err = s.Apply(clocks.Command{Action: "timer_delete", ID: timer.ID}); err != nil {
		t.Fatal(err)
	}
	state, err := s.Apply(clocks.Command{Action: "alarm_save", Name: "Wake", Hour: 7, Minute: 30, Enabled: true})
	if err != nil {
		t.Fatal(err)
	}
	stamp.Wall = state.Alarms[0].Next
	current.Store(stamp)
	if err = s.Tick(); err != nil {
		t.Fatal(err)
	}
	n = waitClockNotice(t, fake.notices)
	if n.actions[0] != "snooze" {
		t.Fatal(n)
	}
	if err = server.Emit(path, iface+".ActionInvoked", n.id, "snooze"); err != nil {
		t.Fatal(err)
	}
	deadline = time.Now().Add(3 * time.Second)
	for time.Now().Before(deadline) {
		if state := s.Snapshot(); len(state.Alerts) == 0 && state.Alarms[0].Snooze > 0 {
			break
		}
		time.Sleep(10 * time.Millisecond)
	}
	if state := s.Snapshot(); len(state.Alerts) != 0 || state.Alarms[0].Snooze != stamp.Wall+300000 {
		t.Fatal(state)
	}
	stamp.Wall += 300000
	stamp.Boot += 300000
	current.Store(stamp)
	if err = s.Tick(); err != nil {
		t.Fatal(err)
	}
	n = waitClockNotice(t, fake.notices)
	// Closing the critical card by hand is the same as Dismiss.
	if err = server.Emit(path, iface+".NotificationClosed", n.id, uint32(2)); err != nil {
		t.Fatal(err)
	}
	deadline = time.Now().Add(3 * time.Second)
	for time.Now().Before(deadline) {
		if len(s.Snapshot().Alerts) == 0 {
			break
		}
		time.Sleep(10 * time.Millisecond)
	}
	if len(s.Snapshot().Alerts) != 0 {
		t.Fatal("closed notification left an alarm ringing")
	}
}
func TestClockChimeIsBoundedMonoPCM(t *testing.T) {
	b := clockChime()
	if string(b[:4]) != "RIFF" || string(b[8:16]) != "WAVEfmt " || binary.LittleEndian.Uint32(b[24:]) != 24000 || binary.LittleEndian.Uint16(b[22:]) != 1 || binary.LittleEndian.Uint32(b[40:]) != uint32(len(b)-44) {
		t.Fatal("invalid WAV")
	}
	peak := int16(0)
	for i := 44; i < len(b); i += 2 {
		sample := int16(binary.LittleEndian.Uint16(b[i:]))
		if sample < 0 {
			sample = -sample
		}
		if sample > peak {
			peak = sample
		}
	}
	if peak < 1000 || peak > 9000 {
		t.Fatal(peak)
	}
}
