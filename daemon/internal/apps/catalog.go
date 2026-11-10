// Package apps is WhiteRose's app setup: the apps a WhiteRose desktop comes
// with, where each one comes from on Ubuntu and on Fedora, and installing
// them. Distro packages where the distro has a real one, the vendor's own
// repository where there is one, Flathub otherwise. No snaps: they cost
// RAM and battery (snapd, a mounted image per app) and don't exist on Fedora.
//
// Installing needs root. The daemon runs `whiterosed apps-root <id>...`
// through pkexec (so the shell's polkit prompt asks for the password); that
// helper only accepts ids from this catalogue, and every URL and command it
// uses is written here.
package apps

import (
	"bufio"
	"os"
	"strings"
)

// Distro is the family the setup knows how to install on.
type Distro string

const (
	Ubuntu Distro = "ubuntu" // and Debian-based
	Fedora Distro = "fedora"
)

// Kind is where a package comes from.
type Kind string

const (
	Native  Kind = "native"  // the distro's own repositories (apt / dnf)
	Vendor  Kind = "vendor"  // the maker's repository, added first
	Copr    Kind = "copr"    // a Fedora COPR (community build)
	Flatpak Kind = "flatpak" // Flathub
)

// Source is how an app is installed on one distro.
type Source struct {
	Kind Kind
	// Package name (apt/dnf) or Flatpak app id.
	Package string
	// Vendor repository (Kind Vendor) or COPR project (Kind Copr).
	Repo string
	// Built by someone other than the app's makers.
	Community bool
}

// App is one app in the setup.
type App struct {
	ID      string
	Name    string
	What    string
	Sources map[Distro]Source
	// A command on $PATH that means it's installed, however it got there
	// (a Flatpak is also found by its id), so the setup never adds a
	// second copy of something you already have.
	Command string
}

// Catalog is every app the setup offers, in the order Settings lists them.
var Catalog = []App{
	{ID: "ghostty", Name: "Ghostty", What: "Terminal", Command: "ghostty", Sources: map[Distro]Source{
		Ubuntu: {Kind: Native, Package: "ghostty"},
		Fedora: {Kind: Copr, Package: "ghostty", Repo: "scottames/ghostty", Community: true},
	}},
	{ID: "zsh", Name: "Zsh and Oh My Zsh", What: "Shell, with your theme", Command: "zsh", Sources: map[Distro]Source{
		Ubuntu: {Kind: Native, Package: "zsh"},
		Fedora: {Kind: Native, Package: "zsh"},
	}},
	{ID: "git", Name: "Git and GitHub CLI", What: "Version control", Command: "gh", Sources: map[Distro]Source{
		Ubuntu: {Kind: Native, Package: "git gh"},
		Fedora: {Kind: Native, Package: "git gh"},
	}},
	{ID: "vscode", Name: "Visual Studio Code", What: "Code editor, with your extensions", Command: "code", Sources: map[Distro]Source{
		Ubuntu: {Kind: Vendor, Package: "code", Repo: "vscode"},
		Fedora: {Kind: Vendor, Package: "code", Repo: "vscode"},
	}},
	{ID: "thunderbird", Name: "Thunderbird", What: "Email", Command: "thunderbird", Sources: map[Distro]Source{
		// Ubuntu's thunderbird .deb only installs the snap.
		Ubuntu: {Kind: Flatpak, Package: "org.mozilla.Thunderbird"},
		Fedora: {Kind: Native, Package: "thunderbird"},
	}},
	{ID: "bitwarden", Command: "bitwarden", Name: "Bitwarden", What: "Passwords", Sources: map[Distro]Source{
		// Updates itself through Flathub (the official .deb/.rpm don't).
		Ubuntu: {Kind: Flatpak, Package: "com.bitwarden.desktop", Community: true},
		Fedora: {Kind: Flatpak, Package: "com.bitwarden.desktop", Community: true},
	}},
	{ID: "libreoffice", Name: "LibreOffice", What: "Office", Command: "libreoffice", Sources: map[Distro]Source{
		Ubuntu: {Kind: Native, Package: "libreoffice"},
		Fedora: {Kind: Native, Package: "libreoffice"},
	}},
	{ID: "vlc", Name: "VLC", What: "Video and music", Command: "vlc", Sources: map[Distro]Source{
		Ubuntu: {Kind: Native, Package: "vlc"},
		Fedora: {Kind: Native, Package: "vlc"},
	}},
	{ID: "calculator", Name: "Calculator", What: "GNOME Calculator", Command: "gnome-calculator", Sources: map[Distro]Source{
		Ubuntu: {Kind: Native, Package: "gnome-calculator"},
		Fedora: {Kind: Native, Package: "gnome-calculator"},
	}},
	{ID: "zapzap", Command: "zapzap", Name: "ZapZap", What: "WhatsApp", Sources: map[Distro]Source{
		Ubuntu: {Kind: Flatpak, Package: "com.rtosta.zapzap"},
		Fedora: {Kind: Flatpak, Package: "com.rtosta.zapzap"},
	}},
	{ID: "discord", Command: "discord", Name: "Discord", What: "Chat", Sources: map[Distro]Source{
		Ubuntu: {Kind: Flatpak, Package: "com.discordapp.Discord", Community: true},
		Fedora: {Kind: Flatpak, Package: "com.discordapp.Discord", Community: true},
	}},
	// Files' cloud accounts (Google Drive, OneDrive, Dropbox, iCloud, S3)
	// are rclone mounts; fuse3 lets them open as folders.
	{ID: "rclone", Name: "rclone", What: "Cloud drives in Files", Command: "rclone", Sources: map[Distro]Source{
		Ubuntu: {Kind: Native, Package: "rclone fuse3"},
		Fedora: {Kind: Native, Package: "rclone fuse3"},
	}},
	{ID: "signal", Name: "Signal", What: "Private messages", Command: "signal-desktop", Sources: map[Distro]Source{
		Ubuntu: {Kind: Vendor, Package: "signal-desktop", Repo: "signal"},
		// Signal has no Fedora package: Flathub's community build.
		Fedora: {Kind: Flatpak, Package: "org.signal.Signal", Community: true},
	}},
}

// Find returns the app with this id.
func Find(id string) (App, bool) {
	for _, a := range Catalog {
		if a.ID == id {
			return a, true
		}
	}
	return App{}, false
}

// Repo is a vendor repository: its signing key and where packages come from.
type Repo struct {
	KeyURL string
	// apt: the key file and the deb822 sources file.
	AptKeyPath, AptSourcesPath, AptSources string
	// dnf: the .repo file.
	DnfRepoPath, DnfRepo string
}

// Repos are the vendor repositories the setup may add, exactly as their
// makers document them (the key kept ASCII-armoured, which apt reads).
var Repos = map[string]Repo{
	"vscode": {
		KeyURL:         "https://packages.microsoft.com/keys/microsoft.asc",
		AptKeyPath:     "/usr/share/keyrings/microsoft.asc",
		AptSourcesPath: "/etc/apt/sources.list.d/vscode.sources",
		AptSources: "Types: deb\nURIs: https://packages.microsoft.com/repos/code\nSuites: stable\nComponents: main\n" +
			"Architectures: amd64,arm64,armhf\nSigned-By: /usr/share/keyrings/microsoft.asc\n",
		DnfRepoPath: "/etc/yum.repos.d/vscode.repo",
		DnfRepo: "[code]\nname=Visual Studio Code\nbaseurl=https://packages.microsoft.com/yumrepos/vscode\nenabled=1\n" +
			"autorefresh=1\ntype=rpm-md\ngpgcheck=1\ngpgkey=https://packages.microsoft.com/keys/microsoft.asc\n",
	},
	"signal": {
		KeyURL:         "https://updates.signal.org/desktop/apt/keys.asc",
		AptKeyPath:     "/usr/share/keyrings/signal-desktop-keyring.asc",
		AptSourcesPath: "/etc/apt/sources.list.d/signal-desktop.sources",
		AptSources: "Types: deb\nURIs: https://updates.signal.org/desktop/apt\nSuites: xenial\nComponents: main\n" +
			"Architectures: amd64\nSigned-By: /usr/share/keyrings/signal-desktop-keyring.asc\n",
	},
}

// FlathubURL is the Flathub remote's definition.
const FlathubURL = "https://dl.flathub.org/repo/flathub.flatpakrepo"

// DetectDistro reads /etc/os-release (ID, then ID_LIKE).
func DetectDistro(osRelease string) (Distro, bool) {
	f, err := os.Open(osRelease)
	if err != nil {
		return "", false
	}
	defer f.Close()
	vals := map[string]string{}
	sc := bufio.NewScanner(f)
	for sc.Scan() {
		k, v, ok := strings.Cut(sc.Text(), "=")
		if ok {
			vals[k] = strings.Trim(v, `"'`)
		}
	}
	if sc.Err() != nil {
		return "", false
	}
	for _, id := range append([]string{vals["ID"]}, strings.Fields(vals["ID_LIKE"])...) {
		switch id {
		case "ubuntu", "debian":
			return Ubuntu, true
		case "fedora", "rhel":
			return Fedora, true
		}
	}
	return "", false
}
