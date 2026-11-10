package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync"

	"whiterose/daemon/internal/apps"
	"whiterose/daemon/internal/fingerprint"
	"whiterose/daemon/internal/server"
)

// cmdFingerprintRoot switches fingerprint sign-in for sudo, login and admin
// prompts (the system's PAM), run through pkexec:
//
//	whiterosed fingerprint-root on|off
func cmdFingerprintRoot(args []string, stdout, stderr io.Writer) int {
	if os.Geteuid() != 0 {
		fmt.Fprintln(stderr, "whiterosed fingerprint-root: run through pkexec (it changes the system's sign-in)")
		return 2
	}
	if len(args) != 1 || (args[0] != "on" && args[0] != "off") {
		fmt.Fprintln(stderr, "usage: whiterosed fingerprint-root on|off")
		return 2
	}
	d, _ := apps.DetectDistro("/etc/os-release")
	cmds, err := fingerprint.LoginCommands(string(d), args[0] == "on")
	if err != nil {
		fmt.Fprintln(stderr, "whiterosed fingerprint-root:", err)
		return 2
	}
	for _, c := range cmds {
		cmd := exec.Command(c[0], c[1:]...)
		// pam-auth-update asks questions on a terminal otherwise.
		cmd.Env = append(os.Environ(), "DEBIAN_FRONTEND=noninteractive")
		cmd.Stdout, cmd.Stderr = stderr, stderr
		if err := cmd.Run(); err != nil {
			fmt.Fprintf(stderr, "whiterosed fingerprint-root: %s: %v\n", strings.Join(c, " "), err)
			return 1
		}
	}
	return 0
}

// rootSelf is what pkexec runs: the installed /usr/bin/whiterosed (the path
// the polkit actions name, so their message shows), else this binary (a
// build being worked on).
func rootSelf() (string, error) {
	self, err := os.Executable()
	if err != nil {
		return "", err
	}
	if strings.HasPrefix(self, "/usr/lib/whiterose/") {
		if _, err := os.Stat("/usr/bin/whiterosed"); err == nil {
			return "/usr/bin/whiterosed", nil
		}
	}
	return filepath.Clean(self), nil
}

// fingerprints: the fprintd connection, kept (an enrolment is cancelled
// from another request).
type fingerprints struct {
	mu        sync.Mutex
	svc       *fingerprint.Service
	broadcast func(any)
}

func (f *fingerprints) service() (*fingerprint.Service, error) {
	f.mu.Lock()
	defer f.mu.Unlock()
	if f.svc == nil {
		s, err := fingerprint.New()
		if err != nil {
			return nil, err
		}
		f.svc = s
	}
	return f.svc, nil
}

func (f *fingerprints) status() (any, error) {
	s, err := f.service()
	if err != nil {
		return nil, err
	}
	st, err := s.Status()
	if err != nil {
		return nil, err
	}
	d, _ := apps.DetectDistro("/etc/os-release")
	return map[string]any{
		"reader": st.Reader, "stages": st.Stages, "scan": st.Scan, "fingers": st.Fingers,
		"login": fingerprint.LoginStatus("/", string(d)),
	}, nil
}

func fingerArg(r server.Request) (string, error) {
	var x struct {
		Finger string `json:"finger"`
	}
	if err := json.Unmarshal(r.Args, &x); err != nil {
		return "", err
	}
	return x.Finger, nil
}

func (f *fingerprints) ops() map[string]func(server.Request) (any, error) {
	return map[string]func(server.Request) (any, error){
		"fingerprints": func(server.Request) (any, error) { return f.status() },
		// Answers when it's done; each touch is a "fingerprint" event.
		"fingerprint_enroll": func(r server.Request) (any, error) {
			finger, err := fingerArg(r)
			if err != nil {
				return nil, err
			}
			s, err := f.service()
			if err != nil {
				return nil, err
			}
			err = s.Enroll(finger, func(p fingerprint.Progress) {
				f.broadcast(map[string]any{"event": "fingerprint", "finger": finger, "state": p.State,
					"stage": p.Stage, "stages": p.Stages, "message": p.Message})
			})
			if err != nil {
				return nil, err
			}
			return f.status()
		},
		"fingerprint_cancel": func(server.Request) (any, error) {
			if s, err := f.service(); err == nil {
				s.Cancel()
			}
			return nil, nil
		},
		"fingerprint_delete": func(r server.Request) (any, error) {
			finger, err := fingerArg(r)
			if err != nil {
				return nil, err
			}
			s, err := f.service()
			if err != nil {
				return nil, err
			}
			if err := s.Delete(finger); err != nil {
				return nil, err
			}
			return f.status()
		},
		// Fingerprint for sign-in, sudo and admin prompts: the system's PAM,
		// switched as root (the polkit prompt asks for the password).
		"fingerprint_login": func(r server.Request) (any, error) {
			self, err := rootSelf()
			if err != nil {
				return nil, err
			}
			arg := "off"
			if r.On {
				arg = "on"
			}
			var stderr bytesTail
			cmd := exec.Command("pkexec", self, "fingerprint-root", arg)
			cmd.Stderr = &stderr
			if err := cmd.Run(); err != nil {
				var exit *exec.ExitError
				if errors.As(err, &exit) && (exit.ExitCode() == 126 || exit.ExitCode() == 127) {
					return nil, errors.New("not allowed (wrong password, or cancelled)")
				}
				return nil, fmt.Errorf("couldn't change it%s", stderr.suffix())
			}
			return f.status()
		},
	}
}
