// Package session listens for logind asking this session to lock or unlock
// (`loginctl lock-session`, idle and lid handling), so the shell's lock
// screen responds to the same requests as GNOME's.
package session

import (
	"os"

	"github.com/godbus/dbus/v5"
)

// Request is "lock" or "unlock".
type Request string

const (
	Lock   Request = "lock"
	Unlock Request = "unlock"
)

// FromSignal maps a logind Session signal name to a request.
func FromSignal(name string) (Request, bool) {
	switch name {
	case "org.freedesktop.login1.Session.Lock":
		return Lock, true
	case "org.freedesktop.login1.Session.Unlock":
		return Unlock, true
	}
	return "", false
}

// Watch calls fn for each lock/unlock request to this process's session,
// until the bus connection fails.
func Watch(fn func(Request)) error {
	conn, err := dbus.ConnectSystemBus()
	if err != nil {
		return err
	}
	defer conn.Close()
	var path dbus.ObjectPath
	manager := conn.Object("org.freedesktop.login1", "/org/freedesktop/login1")
	if err := manager.Call("org.freedesktop.login1.Manager.GetSessionByPID", 0, uint32(os.Getpid())).Store(&path); err != nil {
		return err
	}
	if err := conn.AddMatchSignal(dbus.WithMatchObjectPath(path), dbus.WithMatchInterface("org.freedesktop.login1.Session")); err != nil {
		return err
	}
	signals := make(chan *dbus.Signal, 8)
	conn.Signal(signals)
	for s := range signals {
		if r, ok := FromSignal(s.Name); ok {
			fn(r)
		}
	}
	return nil
}
