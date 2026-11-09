package apps

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
)

// The parts of the setup that are yours, not the system's: run as you,
// after the apps are installed. Your theme and extension list ship with
// WhiteRose in /usr/share/whiterose/profile.

// OhMyZshInstaller is Oh My Zsh's official installer.
const OhMyZshInstaller = "https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/master/tools/install.sh"

var themeLine = regexp.MustCompile(`(?m)^ZSH_THEME=.*$`)

// SetupZsh installs Oh My Zsh (if it isn't there), puts the theme from
// profileDir in place and selects it in ~/.zshrc.
func SetupZsh(ctx context.Context, home, profileDir string, line func(string)) error {
	omz := filepath.Join(home, ".oh-my-zsh")
	if _, err := os.Stat(filepath.Join(omz, "oh-my-zsh.sh")); err != nil {
		script, err := fetch(ctx, OhMyZshInstaller)
		if err != nil {
			return err
		}
		tmp, err := os.CreateTemp("", "ohmyzsh-*.sh")
		if err != nil {
			return err
		}
		defer os.Remove(tmp.Name())
		if _, err := tmp.Write(script); err != nil {
			return err
		}
		tmp.Close()
		cmd := exec.CommandContext(ctx, "sh", tmp.Name(), "--unattended", "--keep-zshrc")
		// Don't start zsh, don't change the shell (the root step does that).
		cmd.Env = append(os.Environ(), "RUNZSH=no", "CHSH=no", "HOME="+home)
		if out, err := cmd.CombinedOutput(); err != nil {
			return errorFrom("Oh My Zsh", out, err)
		}
		line("Oh My Zsh installed")
	}

	// Your theme, as "custom".
	theme, err := os.ReadFile(filepath.Join(profileDir, "custom.zsh-theme"))
	if err == nil {
		dir := filepath.Join(omz, "custom", "themes")
		if err := os.MkdirAll(dir, 0o755); err != nil {
			return err
		}
		if err := os.WriteFile(filepath.Join(dir, "custom.zsh-theme"), theme, 0o644); err != nil {
			return err
		}
	}

	// ~/.zshrc: Oh My Zsh's template if there isn't one, with the theme set.
	rc := filepath.Join(home, ".zshrc")
	text, err := os.ReadFile(rc)
	if err != nil {
		text, err = os.ReadFile(filepath.Join(omz, "templates", "zshrc.zsh-template"))
		if err != nil {
			return err
		}
	}
	s := string(text)
	if themeLine.MatchString(s) {
		s = themeLine.ReplaceAllString(s, `ZSH_THEME="custom"`)
	} else {
		s = "ZSH_THEME=\"custom\"\n" + s
	}
	if err := os.WriteFile(rc, []byte(s), 0o644); err != nil {
		return err
	}
	line("Theme set")

	// zsh doesn't read /etc/profile.d on Ubuntu: start the desktop from tty1
	// in zsh too (see /etc/profile.d/whiterose-autostart.sh).
	zprofile := filepath.Join(home, ".zprofile")
	cur, _ := os.ReadFile(zprofile)
	if !strings.Contains(string(cur), autostartLine) {
		f, err := os.OpenFile(zprofile, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0o644)
		if err != nil {
			return err
		}
		defer f.Close()
		if _, err := f.WriteString("\n# WhiteRose: the desktop starts from the first console\n" + autostartLine + "\n"); err != nil {
			return err
		}
	}
	return nil
}

const autostartLine = "[ -r /etc/profile.d/whiterose-autostart.sh ] && . /etc/profile.d/whiterose-autostart.sh"

// VSCodeExtensions installs the extensions listed in profileDir (one id per
// line) that aren't installed yet.
func VSCodeExtensions(ctx context.Context, profileDir string, line func(string)) error {
	text, err := os.ReadFile(filepath.Join(profileDir, "vscode-extensions.txt"))
	if err != nil {
		return nil // nothing to install
	}
	have := map[string]bool{}
	if out, err := exec.CommandContext(ctx, "code", "--list-extensions").Output(); err == nil {
		for _, e := range strings.Fields(strings.ToLower(string(out))) {
			have[e] = true
		}
	}
	var failed []string
	for _, e := range strings.Fields(string(text)) {
		if strings.HasPrefix(e, "#") || have[strings.ToLower(e)] {
			continue
		}
		if out, err := exec.CommandContext(ctx, "code", "--install-extension", e).CombinedOutput(); err != nil {
			failed = append(failed, e)
			line(errorFrom(e, out, err).Error())
			continue
		}
		line("Installed " + e)
	}
	if len(failed) > 0 {
		return &partial{what: "extensions", items: failed}
	}
	return nil
}

type partial struct {
	what  string
	items []string
}

func (p *partial) Error() string {
	return "some " + p.what + " didn't install: " + strings.Join(p.items, ", ")
}

func errorFrom(what string, out []byte, err error) error {
	lines := strings.Split(strings.TrimSpace(string(out)), "\n")
	if last := strings.TrimSpace(lines[len(lines)-1]); last != "" {
		return &cmdError{what + ": " + last}
	}
	return &cmdError{what + ": " + err.Error()}
}

type cmdError struct{ s string }

func (e *cmdError) Error() string { return e.s }
