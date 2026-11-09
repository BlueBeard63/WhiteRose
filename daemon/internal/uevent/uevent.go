// Package uevent listens to the kernel's device events (the netlink socket
// udev itself reads). A backlight emits one whenever its level changes, from
// brightness keys or from any program, so watching costs nothing at idle.
package uevent

import (
	"bytes"
	"syscall"
)

// Event is one kernel uevent: its action and KEY=value properties.
type Event struct {
	Action string
	Props  map[string]string
}

// Parse decodes a raw uevent: "action@devpath\0KEY=value\0...". Messages
// relayed by udev (they start with "libudev") are not kernel events: nil.
func Parse(msg []byte) *Event {
	parts := bytes.Split(msg, []byte{0})
	if len(parts) == 0 || !bytes.Contains(parts[0], []byte("@")) {
		return nil
	}
	ev := &Event{Props: map[string]string{}}
	for _, p := range parts[1:] {
		if k, v, ok := bytes.Cut(p, []byte("=")); ok {
			ev.Props[string(k)] = string(v)
		}
	}
	ev.Action = ev.Props["ACTION"]
	if ev.Action == "" {
		ev.Action = string(parts[0][:bytes.IndexByte(parts[0], '@')])
	}
	return ev
}

// Listen calls fn for each kernel uevent until reading fails.
func Listen(fn func(*Event)) error {
	fd, err := syscall.Socket(syscall.AF_NETLINK, syscall.SOCK_DGRAM|syscall.SOCK_CLOEXEC, syscall.NETLINK_KOBJECT_UEVENT)
	if err != nil {
		return err
	}
	defer syscall.Close(fd)
	if err := syscall.Bind(fd, &syscall.SockaddrNetlink{Family: syscall.AF_NETLINK, Groups: 1}); err != nil {
		return err
	}
	buf := make([]byte, 16<<10)
	for {
		n, _, err := syscall.Recvfrom(fd, buf, 0)
		if err != nil {
			if err == syscall.EINTR {
				continue
			}
			return err
		}
		if ev := Parse(buf[:n]); ev != nil {
			fn(ev)
		}
	}
}
