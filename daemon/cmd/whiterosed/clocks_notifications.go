package main

import (
	"context"
	"fmt"
	"html"
	"io"
	"time"

	"github.com/godbus/dbus/v5"
	"whiterose/daemon/internal/clocks"
)

// Critical notifications are delivered by the daemon, not the app. D-Bus
// actions acknowledge the same alert that the app and the island show.
func clockNotifications(s *clocks.Service, updates <-chan clocks.State, done <-chan struct{}, log io.Writer) {
	runClockNotifications(s, updates, done, log, clockSound)
}

func runClockNotifications(s *clocks.Service, updates <-chan clocks.State, done <-chan struct{}, log io.Writer, sound func(context.Context, clocks.AlarmOptions, int64) error) {
	var conn *dbus.Conn
	signals := make(chan *dbus.Signal, 32)
	ids := map[uint32]string{}
	owner := ""
	connect := func() {
		if conn != nil && conn.Connected() {
			return
		}
		// A new bus cannot retain notification IDs from the old connection.
		ids = map[uint32]string{}
		owner = ""
		if conn != nil {
			conn.Close()
		}
		c, err := dbus.ConnectSessionBus()
		if err != nil {
			return
		}
		for _, rule := range [][]dbus.MatchOption{
			{dbus.WithMatchInterface("org.freedesktop.Notifications")},
			{dbus.WithMatchInterface("org.freedesktop.DBus"), dbus.WithMatchMember("NameOwnerChanged"), dbus.WithMatchArg(0, "org.freedesktop.Notifications")},
		} {
			_ = c.AddMatchSignal(rule...)
		}
		c.Signal(signals)
		conn = c
	}
	connect()
	defer func() {
		if conn != nil {
			conn.Close()
		}
	}()
	var retry <-chan time.Time
	ringing := map[string]context.CancelFunc{}
	defer func() {
		for _, cancel := range ringing {
			cancel()
		}
	}()
	sync := func() {
		state := s.Snapshot()
		active := map[string]clocks.Alert{}
		for _, a := range state.Alerts {
			active[a.ID] = a
		}
		for id, cancel := range ringing {
			a, exists := active[id]
			if !exists || a.Silenced || a.Options.Volume == 0 {
				cancel()
				delete(ringing, id)
			}
		}
		for _, a := range state.Alerts {
			if _, playing := ringing[a.ID]; playing || a.Silenced || a.Options.Volume == 0 {
				continue
			}
			ctx, cancel := context.WithCancel(context.Background())
			ringing[a.ID] = cancel
			go clockRing(ctx, a, sound, log)
		}
		if len(active) > 0 {
			connect()
		}
		if conn != nil && conn.Connected() && (len(active) > 0 || len(ids) > 0) {
			ctx, cancel := context.WithTimeout(context.Background(), time.Second)
			var current string
			_ = conn.BusObject().CallWithContext(ctx, "org.freedesktop.DBus.GetNameOwner", 0, "org.freedesktop.Notifications").Store(&current)
			cancel()
			if current != owner {
				// A snapshot update can arrive before NameOwnerChanged.
				// Bind IDs to the actual current server, not signal timing.
				owner = current
				ids = map[uint32]string{}
			}
		}
		for notification, alert := range ids {
			if _, ok := active[alert]; !ok {
				delete(ids, notification)
				if conn != nil && conn.Connected() && owner != "" {
					ctx, cancel := context.WithTimeout(context.Background(), time.Second)
					_ = conn.Object(owner, "/org/freedesktop/Notifications").CallWithContext(ctx, "org.freedesktop.Notifications.CloseNotification", 0, notification).Err
					cancel()
				}
			}
		}
		if conn != nil && conn.Connected() && owner != "" {
			for _, a := range state.Alerts {
				exists := false
				for _, id := range ids {
					if id == a.ID {
						exists = true
						break
					}
				}
				if exists {
					continue
				}
				title := a.Name
				actions := []string{"restart", "Restart timer", "dismiss", "Dismiss"}
				if a.Kind == "timer" {
					title += " finished"
				} else {
					actions = []string{"snooze", fmt.Sprintf("Snooze %d min", a.Options.SnoozeDuration/60000), "dismiss", "Dismiss"}
				}
				hints := map[string]dbus.Variant{
					"urgency":        dbus.MakeVariant(byte(2)),
					"desktop-entry":  dbus.MakeVariant("whiterose-clocks"),
					"suppress-sound": dbus.MakeVariant(true),
				}
				var id uint32
				ctx, cancel := context.WithTimeout(context.Background(), time.Second)
				err := conn.Object(owner, "/org/freedesktop/Notifications").CallWithContext(ctx, "org.freedesktop.Notifications.Notify", 0, "Clocks", uint32(0), "whiterose-clocks", title, html.EscapeString(a.Message), actions, hints, int32(0)).Store(&id)
				cancel()
				if err == nil {
					ids[id] = a.ID
				}
			}
		}
		if len(active) == 0 {
			retry = nil
			return
		}
		// Retry delivery after the shell starts/restarts, and repeat the chime
		// while an alert is pending. No polling or subprocess when idle.
		retry = time.After(2 * time.Second)
	}
	sync()
	for {
		select {
		case <-done:
			return
		case <-updates:
			sync()
		case <-retry:
			sync()
		case sig := <-signals:
			if sig == nil {
				continue
			}
			if sig.Name == "org.freedesktop.DBus.NameOwnerChanged" {
				if sig.Sender == "org.freedesktop.DBus" {
					sync()
				}
				continue
			}
			if sig.Sender != owner {
				continue
			}
			if len(sig.Body) < 2 {
				continue
			}
			id, ok := sig.Body[0].(uint32)
			if !ok {
				continue
			}
			alert, ok := ids[id]
			if !ok {
				continue
			}
			action := ""
			if sig.Name == "org.freedesktop.Notifications.ActionInvoked" {
				switch sig.Body[1] {
				case "snooze":
					action = "alert_snooze"
				case "restart":
					action = "alert_restart"
				case "dismiss":
					action = "alert_dismiss"
				}
			} else if sig.Name == "org.freedesktop.Notifications.NotificationClosed" {
				reason, _ := sig.Body[1].(uint32)
				delete(ids, id)
				if reason == 2 {
					action = "alert_dismiss"
				}
			}
			if action != "" {
				_, _ = s.Apply(clocks.Command{Action: action, ID: alert})
			}
			sync()
		}
	}
}

func clockChime() []byte {
	pcm, _ := clocks.SoundPCM("chime", "")
	return clocks.WAV(pcm)
}
