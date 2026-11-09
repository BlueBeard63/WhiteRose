package apps

import (
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
)

func TestEveryAppInstallsOnBothDistros(t *testing.T) {
	seen := map[string]bool{}
	for _, a := range Catalog {
		if seen[a.ID] {
			t.Errorf("%s listed twice", a.ID)
		}
		seen[a.ID] = true
		for _, d := range []Distro{Ubuntu, Fedora} {
			src, ok := a.Sources[d]
			if !ok {
				t.Errorf("%s has no %s source", a.ID, d)
				continue
			}
			if src.Kind == Vendor {
				r, ok := Repos[src.Repo]
				if !ok || (d == Ubuntu && r.AptSources == "") || (d == Fedora && r.DnfRepo == "") {
					t.Errorf("%s: vendor repo %q isn't defined for %s", a.ID, src.Repo, d)
				}
			}
			if (src.Kind == Native || src.Kind == Vendor || src.Kind == Copr) && a.Command == "" {
				t.Errorf("%s: a package needs a command to tell it's installed", a.ID)
			}
		}
	}
}

func TestUbuntuPlanOrder(t *testing.T) {
	steps, err := Plan(Ubuntu, []string{"vscode", "bitwarden", "zsh", "signal", "vlc"}, "jack")
	if err != nil {
		t.Fatal(err)
	}
	var labels []string
	for _, s := range steps {
		labels = append(labels, s.Label)
	}
	want := []string{
		"Installing Flatpak", "Adding Flathub",
		"Adding the Signal repository", "Adding the Visual Studio Code repository",
		"Refreshing package lists", "Installing Visual Studio Code, Zsh and Oh My Zsh, Signal, VLC",
		"Installing Bitwarden", "Making zsh your shell",
	}
	if !reflect.DeepEqual(labels, want) {
		t.Fatalf("steps:\n%s", strings.Join(labels, "\n"))
	}
	if got := steps[5].Cmd; !reflect.DeepEqual(got, []string{"apt-get", "install", "-y", "code", "zsh", "signal-desktop", "vlc"}) {
		t.Errorf("one apt transaction for everything native: %v", got)
	}
	if got := steps[7].Cmd; !reflect.DeepEqual(got, []string{"usermod", "--shell", "/usr/bin/zsh", "jack"}) {
		t.Errorf("login shell: %v", got)
	}
}

func TestFedoraUsesCoprAndFlathubWhereNeeded(t *testing.T) {
	steps, err := Plan(Fedora, []string{"ghostty", "signal", "git"}, "")
	if err != nil {
		t.Fatal(err)
	}
	var cmds []string
	for _, s := range steps {
		cmds = append(cmds, strings.Join(s.Cmd, " ")+s.Repo)
	}
	want := []string{
		"flatpak remote-add --if-not-exists flathub " + FlathubURL,
		"dnf -y copr enable scottames/ghostty",
		"dnf -y install ghostty git gh",
		"flatpak install -y --noninteractive flathub org.signal.Signal",
	}
	if !reflect.DeepEqual(cmds, want) {
		t.Fatalf("steps:\n%s", strings.Join(cmds, "\n"))
	}
}

func TestUnknownAppsAreRefused(t *testing.T) {
	if _, err := Plan(Ubuntu, []string{"vlc", "rm -rf /"}, ""); err == nil {
		t.Fatal("an id outside the catalogue was accepted")
	}
}

func TestDetectDistro(t *testing.T) {
	dir := t.TempDir()
	cases := map[string]Distro{
		"NAME=\"Ubuntu\"\nID=ubuntu\nID_LIKE=debian\n":   Ubuntu,
		"ID=linuxmint\nID_LIKE=\"ubuntu debian\"\n":      Ubuntu,
		"NAME=\"Fedora Linux\"\nID=fedora\n":             Fedora,
		"ID=\"rocky\"\nID_LIKE=\"rhel centos fedora\"\n": Fedora,
	}
	for text, want := range cases {
		p := filepath.Join(dir, "os-release")
		_ = os.WriteFile(p, []byte(text), 0o644)
		if got, ok := DetectDistro(p); !ok || got != want {
			t.Errorf("%q: got %q", text, got)
		}
	}
	_ = os.WriteFile(filepath.Join(dir, "arch"), []byte("ID=arch\n"), 0o644)
	if _, ok := DetectDistro(filepath.Join(dir, "arch")); ok {
		t.Error("Arch isn't supported")
	}
}

func TestRunReportsEachStepAndKeepsGoingAfterAFailure(t *testing.T) {
	steps := []Step{
		{Label: "one", Apps: []string{"vlc"}, Cmd: []string{"sh", "-c", "echo fetching; echo unpacking"}},
		{Label: "two", Apps: []string{"zsh"}, Cmd: []string{"sh", "-c", "echo 'E: Unable to locate package zsh' >&2; exit 100"}},
		{Label: "three", Apps: []string{"git"}, Cmd: []string{"true"}},
	}
	var got []string
	err := Run(t.Context(), Ubuntu, steps, func(e Event) { got = append(got, e.Step+":"+e.State+":"+e.Line+e.Error) })
	if err == nil || !strings.Contains(err.Error(), "two") {
		t.Fatalf("the failure isn't reported: %v", err)
	}
	want := []string{
		"one:running:", "one:running:fetching", "one:running:unpacking", "one:done:",
		"two:running:", "two:running:E: Unable to locate package zsh", "two:failed:sh: E: Unable to locate package zsh",
		"three:running:", "three:done:",
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("events:\n%s", strings.Join(got, "\n"))
	}
}

func TestZshThemeIsInstalledAndSelected(t *testing.T) {
	home, profile := t.TempDir(), t.TempDir()
	omz := filepath.Join(home, ".oh-my-zsh")
	_ = os.MkdirAll(filepath.Join(omz, "templates"), 0o755)
	_ = os.WriteFile(filepath.Join(omz, "oh-my-zsh.sh"), []byte("#"), 0o644)
	_ = os.WriteFile(filepath.Join(omz, "templates", "zshrc.zsh-template"), []byte("export ZSH=\"$HOME/.oh-my-zsh\"\nZSH_THEME=\"robbyrussell\"\nplugins=(git)\n"), 0o644)
	_ = os.WriteFile(filepath.Join(profile, "custom.zsh-theme"), []byte("PROMPT='%~ ❯ '\n"), 0o644)

	if err := SetupZsh(t.Context(), home, profile, func(string) {}); err != nil {
		t.Fatal(err)
	}
	theme, _ := os.ReadFile(filepath.Join(omz, "custom", "themes", "custom.zsh-theme"))
	rc, _ := os.ReadFile(filepath.Join(home, ".zshrc"))
	if string(theme) != "PROMPT='%~ ❯ '\n" || !strings.Contains(string(rc), `ZSH_THEME="custom"`) || strings.Contains(string(rc), "robbyrussell") {
		t.Fatalf("theme %q\nzshrc %q", theme, rc)
	}
	// An existing .zshrc keeps everything but the theme line.
	_ = os.WriteFile(filepath.Join(home, ".zshrc"), []byte("alias ll='ls -l'\nZSH_THEME=\"agnoster\"\n"), 0o644)
	_ = SetupZsh(t.Context(), home, profile, func(string) {})
	rc, _ = os.ReadFile(filepath.Join(home, ".zshrc"))
	if string(rc) != "alias ll='ls -l'\nZSH_THEME=\"custom\"\n" {
		t.Fatalf("existing zshrc: %q", rc)
	}
	// The console autostart, once.
	zp, _ := os.ReadFile(filepath.Join(home, ".zprofile"))
	if strings.Count(string(zp), autostartLine) != 1 {
		t.Fatalf("zprofile: %q", zp)
	}
}
