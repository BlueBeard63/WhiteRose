package fingerprint

import (
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
)

// Using a finger for sign-in, sudo and admin (polkit) prompts is the
// system's PAM: Ubuntu's common-auth (pam-auth-update's "fprintd" profile)
// or Fedora's system-auth (authselect's with-fingerprint feature). Turning
// it on or off changes it for everyone on this computer, so it runs as root
// (whiterosed fingerprint-root, through pkexec). The lock screen doesn't
// need it: it asks for a finger itself.

// Login is what the system does now.
type Login struct {
	Supported bool `json:"supported"` // it can be switched here
	Enabled   bool `json:"enabled"`
}

// The PAM file that decides, and how it's switched, per distribution
// family ("ubuntu" or "fedora", as apps.DetectDistro says).
type loginWay struct {
	file   string                 // under /etc/pam.d
	marker string                 // what's there when it's on
	can    func(root string) bool // the tool exists
	cmd    func(on bool) [][]string
}

var ways = map[string]loginWay{
	"ubuntu": {
		file:   "common-auth",
		marker: "pam_fprintd.so",
		can: func(root string) bool {
			_, err := os.Stat(filepath.Join(root, "usr/share/pam-configs/fprintd"))
			return err == nil && have("pam-auth-update")
		},
		cmd: func(on bool) [][]string {
			if on {
				return [][]string{{"pam-auth-update", "--enable", "fprintd"}}
			}
			return [][]string{{"pam-auth-update", "--remove", "fprintd"}}
		},
	},
	"fedora": {
		file:   "system-auth",
		marker: "pam_fprintd.so",
		can: func(root string) bool {
			// authselect manages PAM only once a profile is selected.
			_, err := os.Stat(filepath.Join(root, "etc/authselect/authselect.conf"))
			return err == nil && have("authselect")
		},
		cmd: func(on bool) [][]string {
			if on {
				return [][]string{{"authselect", "enable-feature", "with-fingerprint"}}
			}
			return [][]string{{"authselect", "disable-feature", "with-fingerprint"}}
		},
	},
}

var have = func(name string) bool { _, err := exec.LookPath(name); return err == nil }

// LoginStatus reads PAM (under `root`, "/" normally).
func LoginStatus(root, distro string) Login {
	w, ok := ways[distro]
	if !ok {
		return Login{}
	}
	b, _ := os.ReadFile(filepath.Join(root, "etc/pam.d", w.file))
	on := false
	for _, l := range strings.Split(string(b), "\n") {
		l = strings.TrimSpace(l)
		if !strings.HasPrefix(l, "#") && strings.Contains(l, w.marker) {
			on = true
		}
	}
	return Login{Supported: w.can(root), Enabled: on}
}

// LoginCommands are what fingerprint-root runs (as root).
func LoginCommands(distro string, on bool) ([][]string, error) {
	w, ok := ways[distro]
	if !ok {
		return nil, errors.New("this distribution isn't supported (Ubuntu or Fedora)")
	}
	return w.cmd(on), nil
}
