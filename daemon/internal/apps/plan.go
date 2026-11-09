package apps

import (
	"fmt"
	"sort"
	"strings"
)

// Step is one thing the root helper does: run a command, or put a vendor
// repository in place (download its key, write its sources file).
type Step struct {
	// What Settings shows while it runs.
	Label string
	// The apps this step is for (their status follows it).
	Apps []string
	// A command, run without a shell.
	Cmd []string
	// A vendor repository to add (instead of Cmd).
	Repo string
}

// Plan is everything to do as root for `ids` on `d`, in order: Flathub if
// any app comes from there, vendor repositories and COPRs, one package
// transaction for everything native, then the Flatpaks, then zsh as the
// login shell for `user`.
func Plan(d Distro, ids []string, user string) ([]Step, error) {
	var native, natApps, flat []string
	flatApps := map[string]string{}
	repos := map[string][]string{}
	coprs := map[string][]string{}
	wantZsh := false
	for _, id := range ids {
		a, ok := Find(id)
		if !ok {
			return nil, fmt.Errorf("unknown app %q", id)
		}
		src, ok := a.Sources[d]
		if !ok {
			return nil, fmt.Errorf("%s isn't available on %s", a.Name, d)
		}
		switch src.Kind {
		case Native, Vendor, Copr:
			native = append(native, strings.Fields(src.Package)...)
			natApps = append(natApps, id)
			if src.Kind == Vendor {
				repos[src.Repo] = append(repos[src.Repo], id)
			}
			if src.Kind == Copr {
				coprs[src.Repo] = append(coprs[src.Repo], id)
			}
		case Flatpak:
			flat = append(flat, src.Package)
			flatApps[src.Package] = id
		}
		if id == "zsh" {
			wantZsh = true
		}
	}

	var steps []Step
	if len(flat) > 0 {
		apps := sortedValues(flatApps)
		if d == Ubuntu {
			steps = append(steps, Step{Label: "Installing Flatpak", Apps: apps, Cmd: aptInstall("flatpak")})
		}
		steps = append(steps, Step{Label: "Adding Flathub", Apps: apps,
			Cmd: []string{"flatpak", "remote-add", "--if-not-exists", "flathub", FlathubURL}})
	}
	for _, r := range sortedKeys(repos) {
		steps = append(steps, Step{Label: "Adding the " + repoName(r) + " repository", Apps: repos[r], Repo: r})
	}
	for _, c := range sortedKeys(coprs) {
		steps = append(steps, Step{Label: "Adding " + c + " (COPR)", Apps: coprs[c], Cmd: []string{"dnf", "-y", "copr", "enable", c}})
	}
	if len(native) > 0 {
		if d == Ubuntu {
			steps = append(steps, Step{Label: "Refreshing package lists", Apps: natApps, Cmd: []string{"apt-get", "update"}})
			steps = append(steps, Step{Label: "Installing " + joinNames(natApps), Apps: natApps, Cmd: aptInstall(native...)})
		} else {
			steps = append(steps, Step{Label: "Installing " + joinNames(natApps), Apps: natApps,
				Cmd: append([]string{"dnf", "-y", "install"}, native...)})
		}
	}
	for _, f := range flat {
		a, _ := Find(flatApps[f])
		steps = append(steps, Step{Label: "Installing " + a.Name, Apps: []string{a.ID},
			Cmd: []string{"flatpak", "install", "-y", "--noninteractive", "flathub", f}})
	}
	if wantZsh && user != "" {
		steps = append(steps, Step{Label: "Making zsh your shell", Apps: []string{"zsh"},
			Cmd: []string{"usermod", "--shell", "/usr/bin/zsh", user}})
	}
	return steps, nil
}

func aptInstall(pkgs ...string) []string {
	// Recommends included: LibreOffice and VLC need theirs to look and work right.
	return append([]string{"apt-get", "install", "-y"}, pkgs...)
}

func repoName(r string) string {
	switch r {
	case "vscode":
		return "Visual Studio Code"
	case "signal":
		return "Signal"
	}
	return r
}

func joinNames(ids []string) string {
	var names []string
	for _, id := range ids {
		a, _ := Find(id)
		names = append(names, a.Name)
	}
	return strings.Join(names, ", ")
}

func sortedKeys(m map[string][]string) []string {
	var k []string
	for x := range m {
		k = append(k, x)
	}
	sort.Strings(k)
	return k
}

func sortedValues(m map[string]string) []string {
	var v []string
	for _, x := range m {
		v = append(v, x)
	}
	sort.Strings(v)
	return v
}
