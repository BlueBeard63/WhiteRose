# Changelog

Every WhiteRose release, newest first. Written from `project.yaml` by `make changelog`; edit that, not this.

## 0.2.1 · Island views, one-command install (2026-11-11)

A smaller release: an easier way to install WhiteRose, and the centre island's views reworked so new ones are one file each.

### Added

- One command installs WhiteRose: it downloads both packages, checks their signature and installs them (install.sh, attached to every release)

### Changed

- Each of the island's views (dock, clipboard, screenshots, launcher, switcher, the recovery prompt) is declared in its own file; the island sizes and morphs to whatever it shows

### Fixed

- Installing just the whiterose package on its own stopped with a missing whiterose-core; the installer and the release notes now install the pair

Contributors: @BlueBeard63

## 0.2.0 · Files (2026-11-11)

The first WhiteRose release, and it ships two milestones at once: 0.1 Foundations (the compositor, shell, Settings, Files and Notepad, signed updates, fingerprint unlock) and 0.2 Files.

### Added

- Files without dialogs: the Sheet beside your files holds info, renaming many at once, share and compress, connecting, and compare & sync
- Cloud drives in Files: Google Drive, OneDrive, Dropbox, iCloud Drive, pCloud and S3 (through rclone), plus GNOME Online Accounts' drives
- Nextcloud in Files: sync badges on every file, keep offline, free up space, share links
- Network shares: SMB, SFTP, FTP, WebDAV and NFS, hosts from your SSH config, passwords in the keyring
- Compare & sync folders, with the differences marked on the files in both panes
- Your own actions in the right-click menu; search inside files; whiterose-files --find from a terminal

### Changed

- Copies, moves and clashes go in an Activity drawer; questions ask in a strip above the files; new folders are named right where they appear
- Settings asks in place too: questions as a strip, forms and a network's settings as their own page, a Wi-Fi password right under the network
- Notepad asks about unsaved changes in a strip, and Go to line is a bar
- A cleaner look: no chips, rows that don't change height, WhiteRose's own window controls
- Files shows the git branch you're in; git itself stays in your terminal and editor

### Removed

- The gallery view (pictures open as a grid of thumbnails)

Contributors: @BlueBeard63
