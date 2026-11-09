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
	path, err := Find(conn)
	if err != nil {
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

// Find picks this session: the one this process belongs to; failing that
// (started outside a login session, e.g. from a service manager), logind's
// "auto" session; failing that, the user's graphical session.
func Find(conn *dbus.Conn) (dbus.ObjectPath, error) {
	manager := conn.Object("org.freedesktop.login1", "/org/freedesktop/login1")
	var path dbus.ObjectPath
	err := manager.Call("org.freedesktop.login1.Manager.GetSessionByPID", 0, uint32(os.Getpid())).Store(&path)
	if err == nil {
		return path, nil
	}
	if e := manager.Call("org.freedesktop.login1.Manager.GetSession", 0, "auto").Store(&path); e == nil {
		return path, nil
	}
	var user dbus.ObjectPath
	if e := manager.Call("org.freedesktop.login1.Manager.GetUser", 0, uint32(os.Getuid())).Store(&user); e == nil {
		if v, e := conn.Object("org.freedesktop.login1", user).GetProperty("org.freedesktop.login1.User.Display"); e == nil {
			if d, ok := v.Value().([]any); ok && len(d) == 2 {
				if p, ok := d[1].(dbus.ObjectPath); ok && p != "/" {
					return p, nil
				}
			}
		}
	}
	return "", err
}
