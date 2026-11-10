// Package fingerprint manages your fingerprints through fprintd
// (net.reactivated.Fprint on the system bus): the reader, which fingers are
// enrolled, enrolling one (the reader asks for several touches), removing.
//
// fprintd checks permission itself: enrolling and removing are
// "auth_self_keep", so calls allow interactive authorization and the
// session's polkit agent (the WhiteRose shell) asks for your password first.
// Everything is for the calling user (fprintd's "" user name).
//
// Signing in with a finger is separate: the system's PAM decides whether
// login, sudo and admin prompts ask for one (login.go).
package fingerprint

import (
	"errors"
	"slices"
	"strings"
	"sync"
	"time"

	"github.com/godbus/dbus/v5"
)

const (
	busName    = "net.reactivated.Fprint"
	managerObj = "/net/reactivated/Fprint/Manager"
	managerIfc = "net.reactivated.Fprint.Manager"
	deviceIfc  = "net.reactivated.Fprint.Device"
)

// Fingers fprintd knows, in the order the page shows them.
var Fingers = []string{
	"right-index-finger", "left-index-finger",
	"right-thumb", "left-thumb",
	"right-middle-finger", "left-middle-finger",
	"right-ring-finger", "left-ring-finger",
	"right-little-finger", "left-little-finger",
}

func validFinger(f string) bool { return slices.Contains(Fingers, f) }

// Status is what the page shows. Reader is "" when there's none.
type Status struct {
	Reader  string   `json:"reader"`
	Stages  int      `json:"stages"`
	Scan    string   `json:"scan"` // "press" or "swipe"
	Fingers []string `json:"fingers"`
}

// Progress of an enrolment.
type Progress struct {
	State   string `json:"state"` // "stage", "retry", "done", "failed", "cancelled"
	Stage   int    `json:"stage"`
	Stages  int    `json:"stages"`
	Message string `json:"message"`
}

type Service struct {
	conn *dbus.Conn
	// Enrolling: closing it stops the enrolment.
	mu     sync.Mutex
	cancel chan struct{}
	// Give up on an enrolment nobody touches the reader for.
	Idle time.Duration
}

// New connects to the system bus.
func New() (*Service, error) {
	conn, err := dbus.ConnectSystemBus()
	if err != nil {
		return nil, err
	}
	return NewOn(conn), nil
}

// NewOn uses `conn` (tests: a private bus with a fake fprintd).
func NewOn(conn *dbus.Conn) *Service { return &Service{conn: conn, Idle: 2 * time.Minute} }

func (s *Service) Close() { _ = s.conn.Close() }

// friendly turns fprintd's errors into something to show.
func friendly(err error) error {
	if err == nil {
		return nil
	}
	name := errName(err)
	switch {
	case strings.HasSuffix(name, ".PermissionDenied"), strings.HasSuffix(name, ".NotAuthorized"):
		return errors.New("not allowed (wrong password, or cancelled)")
	case strings.HasSuffix(name, ".AlreadyInUse"):
		return errors.New("the reader is busy (the lock screen or an app is using it); try again in a moment")
	case strings.HasSuffix(name, ".NoSuchDevice"):
		return errors.New("there's no fingerprint reader")
	case strings.HasSuffix(name, ".ServiceUnknown"):
		return errors.New("fingerprints aren't available (fprintd isn't installed)")
	case strings.HasSuffix(name, ".PrintsNotDeleted"):
		return errors.New("the fingerprint couldn't be removed from the reader")
	}
	return err
}

// errName is a D-Bus error's name ("" for other errors).
func errName(err error) string {
	var v dbus.Error
	if errors.As(err, &v) {
		return v.Name
	}
	var p *dbus.Error
	if errors.As(err, &p) && p != nil {
		return p.Name
	}
	return ""
}

func (s *Service) device() (dbus.BusObject, error) {
	var p dbus.ObjectPath
	if err := s.conn.Object(busName, managerObj).Call(managerIfc+".GetDefaultDevice", 0).Store(&p); err != nil {
		return nil, friendly(err)
	}
	return s.conn.Object(busName, p), nil
}

// Status: the reader and your enrolled fingers. No reader isn't an error.
func (s *Service) Status() (Status, error) {
	st := Status{Fingers: []string{}}
	dev, err := s.device()
	if err != nil {
		if strings.Contains(err.Error(), "no fingerprint reader") {
			return st, nil
		}
		return st, err
	}
	if v, err := dev.GetProperty(deviceIfc + ".name"); err == nil {
		st.Reader, _ = v.Value().(string)
	}
	if v, err := dev.GetProperty(deviceIfc + ".num-enroll-stages"); err == nil {
		n, _ := v.Value().(int32)
		st.Stages = int(n)
	}
	if v, err := dev.GetProperty(deviceIfc + ".scan-type"); err == nil {
		st.Scan, _ = v.Value().(string)
	}
	var fingers []string
	if err := dev.Call(deviceIfc+".ListEnrolledFingers", 0, "").Store(&fingers); err != nil {
		if strings.HasSuffix(errName(err), ".NoEnrolledPrints") {
			return st, nil
		}
		return st, friendly(err)
	}
	// In the page's order.
	for _, f := range Fingers {
		if slices.Contains(fingers, f) {
			st.Fingers = append(st.Fingers, f)
		}
	}
	return st, nil
}

// claim takes the reader for a change (asking for your password if fprintd
// wants it), runs `fn`, and gives it back.
func (s *Service) claim(fn func(dev dbus.BusObject) error) error {
	dev, err := s.device()
	if err != nil {
		return err
	}
	if err := dev.Call(deviceIfc+".Claim", dbus.FlagAllowInteractiveAuthorization, "").Err; err != nil {
		return friendly(err)
	}
	defer dev.Call(deviceIfc+".Release", 0)
	return fn(dev)
}

// Delete removes one enrolled finger.
func (s *Service) Delete(finger string) error {
	if !validFinger(finger) {
		return errors.New("unknown finger " + finger)
	}
	return s.claim(func(dev dbus.BusObject) error {
		return friendly(dev.Call(deviceIfc+".DeleteEnrolledFinger", dbus.FlagAllowInteractiveAuthorization, finger).Err)
	})
}

// What each enrolment result means for you.
var results = map[string]Progress{
	"enroll-stage-passed":        {State: "stage"},
	"enroll-completed":           {State: "done", Message: "Fingerprint added"},
	"enroll-retry-scan":          {State: "retry", Message: "Didn't catch that, touch the reader again"},
	"enroll-swipe-too-short":     {State: "retry", Message: "Swipe was too short, try again"},
	"enroll-finger-not-centered": {State: "retry", Message: "Put your finger in the middle of the reader"},
	"enroll-remove-and-retry":    {State: "retry", Message: "Lift your finger, then touch the reader again"},
	"enroll-duplicate":           {State: "failed", Message: "That finger is already added"},
	"enroll-data-full":           {State: "failed", Message: "The reader is full: remove a fingerprint first"},
	"enroll-disconnected":        {State: "failed", Message: "The reader was disconnected"},
	"enroll-failed":              {State: "failed", Message: "Couldn't add the fingerprint, try again"},
	"enroll-unknown-error":       {State: "failed", Message: "Couldn't add the fingerprint, try again"},
}

// Enroll adds `finger`: the reader asks for several touches, each reported.
// Blocks until it's done, failed or cancelled (Cancel, or nobody touching
// the reader for s.Idle). One at a time.
func (s *Service) Enroll(finger string, report func(Progress)) error {
	if !validFinger(finger) {
		return errors.New("unknown finger " + finger)
	}
	s.mu.Lock()
	if s.cancel != nil {
		s.mu.Unlock()
		return errors.New("already adding a fingerprint")
	}
	cancel := make(chan struct{})
	s.cancel = cancel
	s.mu.Unlock()
	defer func() {
		s.mu.Lock()
		s.cancel = nil
		s.mu.Unlock()
	}()

	return s.claim(func(dev dbus.BusObject) error {
		stages := 0
		if v, err := dev.GetProperty(deviceIfc + ".num-enroll-stages"); err == nil {
			n, _ := v.Value().(int32)
			stages = int(n)
		}
		match := []dbus.MatchOption{
			dbus.WithMatchObjectPath(dev.Path()),
			dbus.WithMatchInterface(deviceIfc),
			dbus.WithMatchMember("EnrollStatus"),
		}
		if err := s.conn.AddMatchSignal(match...); err != nil {
			return err
		}
		defer s.conn.RemoveMatchSignal(match...)
		sigs := make(chan *dbus.Signal, 16)
		s.conn.Signal(sigs)
		defer s.conn.RemoveSignal(sigs)

		if err := dev.Call(deviceIfc+".EnrollStart", dbus.FlagAllowInteractiveAuthorization, finger).Err; err != nil {
			return friendly(err)
		}
		stage := 0
		report(Progress{State: "stage", Stage: 0, Stages: stages})
		idle := time.NewTimer(s.Idle)
		defer idle.Stop()
		stop := func(state, msg string) error {
			dev.Call(deviceIfc+".EnrollStop", 0)
			report(Progress{State: state, Stage: stage, Stages: stages, Message: msg})
			if state == "cancelled" {
				return nil
			}
			return errors.New(msg)
		}
		for {
			select {
			case <-cancel:
				return stop("cancelled", "")
			case <-idle.C:
				return stop("failed", "Stopped: the reader wasn't touched for a while")
			case sig := <-sigs:
				if sig.Path != dev.Path() || sig.Name != deviceIfc+".EnrollStatus" || len(sig.Body) < 2 {
					continue
				}
				result, _ := sig.Body[0].(string)
				done, _ := sig.Body[1].(bool)
				idle.Reset(s.Idle)
				p, ok := results[result]
				if !ok {
					p = results["enroll-unknown-error"]
				}
				if p.State == "stage" {
					stage++
				}
				if p.State == "done" {
					stage = stages
				}
				p.Stage, p.Stages = stage, stages
				if done || p.State == "done" || p.State == "failed" {
					dev.Call(deviceIfc+".EnrollStop", 0)
					report(p)
					if p.State == "done" {
						return nil
					}
					return errors.New(p.Message)
				}
				report(p)
			}
		}
	})
}

// Cancel stops an enrolment in progress (nothing if there isn't one).
func (s *Service) Cancel() {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.cancel != nil {
		close(s.cancel)
		s.cancel = nil
	}
}
