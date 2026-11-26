package main

import (
	"context"
	"encoding/binary"
	"fmt"
	"html"
	"io"
	"math"
	"os"
	"os/exec"
	"path/filepath"
	"time"

	"github.com/godbus/dbus/v5"
	"whiterose/daemon/internal/clocks"
)

// Critical notifications are delivered by the daemon, not the app. D-Bus
// actions acknowledge the same alert that the app and the island show.
func clockNotifications(s *clocks.Service, updates <-chan clocks.State, done <-chan struct{}, log io.Writer) {
	runClockNotifications(s, updates, done, log, clockSound)
}

func runClockNotifications(s *clocks.Service, updates <-chan clocks.State, done <-chan struct{}, log io.Writer, sound func(context.Context) error) {
	var conn *dbus.Conn
	signals := make(chan *dbus.Signal, 32)
	ids := map[uint32]string{}
	connect := func() {
		if conn != nil && conn.Connected() {
			return
		}
		// A new bus cannot retain notification IDs from the old connection.
		ids = map[uint32]string{}
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
	var soundCancel context.CancelFunc
	defer func() {
		if soundCancel != nil {
			soundCancel()
		}
	}()
	lastSound := time.Time{}
	sync := func() {
		state := s.Snapshot()
		active := map[string]clocks.Alert{}
		for _, a := range state.Alerts {
			active[a.ID] = a
		}
		if len(active) > 0 {
			connect()
		}
		for notification, alert := range ids {
			if _, ok := active[alert]; !ok {
				delete(ids, notification)
				if conn != nil && conn.Connected() {
					ctx, cancel := context.WithTimeout(context.Background(), time.Second)
					_ = conn.Object("org.freedesktop.Notifications", "/org/freedesktop/Notifications").CallWithContext(ctx, "org.freedesktop.Notifications.CloseNotification", 0, notification).Err
					cancel()
				}
			}
		}
		if conn != nil && conn.Connected() {
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
					actions = []string{"snooze", "Snooze 5 min", "dismiss", "Dismiss"}
				}
				hints := map[string]dbus.Variant{
					"urgency":        dbus.MakeVariant(byte(2)),
					"desktop-entry":  dbus.MakeVariant("whiterose-clocks"),
					"suppress-sound": dbus.MakeVariant(true),
				}
				var id uint32
				ctx, cancel := context.WithTimeout(context.Background(), time.Second)
				err := conn.Object("org.freedesktop.Notifications", "/org/freedesktop/Notifications").CallWithContext(ctx, "org.freedesktop.Notifications.Notify", 0, "Clocks", uint32(0), "whiterose-clocks", title, html.EscapeString(a.Message), actions, hints, int32(0)).Store(&id)
				cancel()
				if err == nil {
					ids[id] = a.ID
				}
			}
		}
		if len(active) == 0 {
			retry = nil
			if soundCancel != nil {
				soundCancel()
				soundCancel = nil
			}
			return
		}
		// Retry delivery after the shell starts/restarts, and repeat the chime
		// while an alert is pending. No polling or subprocess when idle.
		retry = time.After(2 * time.Second)
		if time.Since(lastSound) >= 4*time.Second {
			if soundCancel != nil {
				soundCancel()
			}
			ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
			soundCancel = cancel
			lastSound = time.Now()
			go func() {
				if err := sound(ctx); err != nil && ctx.Err() == nil {
					fmt.Fprintln(log, "whiterosed: alarm sound:", err)
				}
			}()
		}
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
				ids = map[uint32]string{}
				// Don't send between the loss and acquisition signals: the
				// replacement may already own the name, yielding an orphaned
				// notification when the acquisition resets our IDs again.
				if len(sig.Body) >= 3 && sig.Body[2] != "" {
					sync()
				} else if len(s.Snapshot().Alerts) > 0 {
					retry = time.After(2 * time.Second)
				}
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

func clockSound(ctx context.Context) error {
	dir := os.Getenv("XDG_CACHE_HOME")
	if dir == "" {
		dir = filepath.Join(os.Getenv("HOME"), ".cache")
	}
	dir = filepath.Join(dir, "whiterose/clocks")
	if err := os.MkdirAll(dir, 0o700); err != nil {
		return err
	}
	path := filepath.Join(dir, "chime.wav")
	if _, err := os.Stat(path); err != nil {
		if err := os.WriteFile(path, clockChime(), 0o600); err != nil {
			return err
		}
	}
	for _, player := range []string{"pw-play", "paplay"} {
		if bin, err := exec.LookPath(player); err == nil {
			return exec.CommandContext(ctx, bin, path).Run()
		}
	}
	return fmt.Errorf("no PipeWire or PulseAudio sound player installed")
}

// A short, enveloped two-tone chime; generated locally, with no downloaded
// sound file or theme dependency.
func clockChime() []byte {
	const rate = 24000
	const count = rate
	b := make([]byte, 44+count*2)
	copy(b, "RIFF")
	binary.LittleEndian.PutUint32(b[4:], uint32(len(b)-8))
	copy(b[8:], "WAVEfmt ")
	binary.LittleEndian.PutUint32(b[16:], 16)
	binary.LittleEndian.PutUint16(b[20:], 1)
	binary.LittleEndian.PutUint16(b[22:], 1)
	binary.LittleEndian.PutUint32(b[24:], rate)
	binary.LittleEndian.PutUint32(b[28:], rate*2)
	binary.LittleEndian.PutUint16(b[32:], 2)
	binary.LittleEndian.PutUint16(b[34:], 16)
	copy(b[36:], "data")
	binary.LittleEndian.PutUint32(b[40:], count*2)
	for i := 0; i < count; i++ {
		t := float64(i) / rate
		local := math.Mod(t, 0.5)
		if local > 0.30 {
			continue
		}
		envelope := math.Min(1, local/0.02) * math.Min(1, (0.30-local)/0.08)
		frequency := 660.0
		if t >= 0.5 {
			frequency = 880
		}
		sample := int16(9000 * envelope * math.Sin(2*math.Pi*frequency*t))
		binary.LittleEndian.PutUint16(b[44+i*2:], uint16(sample))
	}
	return b
}
