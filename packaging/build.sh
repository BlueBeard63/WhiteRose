#!/bin/sh
# Builds WhiteRose and its package inside a distro container (see
# packaging/Dockerfile.*). Run by `make package`; the source is mounted at
# /src (read-only), packages go to /out.
#
#   build.sh deb|rpm VERSION
set -eu
FORMAT="$1"
VERSION="$2"
QS_REV=4f508be500dea6e5732cc3d50382a0048b17e7b1   # QuickShell 0.3.2, what the shell is written against
NFPM_VERSION=2.43.0

work=/work
stage=$work/stage
rm -rf "$work/src" "$stage"
mkdir -p "$work" "$stage" /out

# A clean copy of the source (never build in the mounted tree).
tar -C /src --exclude=./compositor/target --exclude=./apps/target --exclude=./build --exclude=./.git -cf - . | (mkdir -p "$work/src" && tar -C "$work/src" -xf -)
cd "$work/src"

# ---- QuickShell, pinned, against this distro's Qt ----------------------------
if [ ! -x "$work/qs/build/src/quickshell" ] || [ "$(cat "$work/qs/.rev" 2>/dev/null)" != "$QS_REV" ]; then
    rm -rf "$work/qs"
    git clone --quiet https://git.outfoxxed.me/quickshell/quickshell "$work/qs"
    git -C "$work/qs" checkout --quiet "$QS_REV"
    cmake -S "$work/qs" -B "$work/qs/build" -GNinja -DCMAKE_BUILD_TYPE=Release \
        -DDISTRIBUTOR="WhiteRose" -DCRASH_HANDLER=OFF -DX11=OFF -DHYPRLAND=OFF -DI3=OFF \
        -DSERVICE_GREETD=OFF -DINSTALL_QML_PREFIX=lib/whiterose/qml
    cmake --build "$work/qs/build"
    echo "$QS_REV" > "$work/qs/.rev"
fi
qs_bin=$(find "$work/qs/build" -maxdepth 2 -type f -name quickshell -perm -u+x | head -1)

# ---- compositor and daemon ---------------------------------------------------
# The Rust build cache outlives the fresh source copy, so rebuilds are quick.
(cd compositor && CARGO_TARGET_DIR="$work/target" cargo build --release --locked)
# WhiteRose's own apps (Qt Quick + Rust).
(cd apps && CARGO_TARGET_DIR="$work/apps-target" cargo build --release --locked)
(cd daemon && CGO_ENABLED=0 go build -trimpath -ldflags "-s -w" -o "$work/whiterosed" ./cmd/whiterosed)
(cd daemon && CGO_ENABLED=0 go build -trimpath -ldflags "-s -w" -o "$work/whiterose-launch" ./cmd/whiterose-launch)

# ---- the file layout ---------------------------------------------------------
# Two packages. whiterose-core-<version>: everything that runs, in
# /usr/lib/whiterose/<version>/, so versions install side by side (the one
# before stays to go back to). whiterose: what doesn't change between
# versions (the session launcher, commands that run the right version,
# desktop entries, icons, system files); it needs its version's core.
core="$stage/core"
meta="$stage/meta"
v="$core/usr/lib/whiterose/$VERSION"
install -Dm755 "$work/target/release/whiterose-wm" "$v/bin/whiterose-wm"
install -Dm755 "$work/whiterosed" "$v/bin/whiterosed"
install -Dm755 "$work/apps-target/release/whiterose-notepad" "$v/bin/whiterose-notepad"
install -Dm755 "$work/apps-target/release/whiterose-files" "$v/bin/whiterose-files"
install -Dm755 "$qs_bin" "$v/bin/quickshell"
ln -s quickshell "$v/bin/qs"
mkdir -p "$v"
tar -C . --exclude=./shell/tests -cf - shell | tar -C "$v" -xf -
echo "$VERSION" > "$v/VERSION"

install -Dm755 packaging/files/whiterose-select "$meta/usr/lib/whiterose/select"
# Picks the version each login and health-checks a new one (going back if
# it doesn't come up): outside the versions, as it chooses between them.
install -Dm755 "$work/whiterose-launch" "$meta/usr/lib/whiterose/launch"
install -Dm755 packaging/files/whiterose-session "$meta/usr/bin/whiterose-session"
# /usr/bin commands: run the version this session uses.
for b in whiterose-wm whiterosed whiterose-notepad whiterose-files; do
    printf '#!/bin/sh\nd=$(/usr/lib/whiterose/select) || exit 1\nexec "$d/bin/%s" "$@"\n' "$b" > "$meta/usr/bin/$b"
    chmod 755 "$meta/usr/bin/$b"
done
printf '#!/bin/sh\nd=$(/usr/lib/whiterose/select) || exit 1\nPATH="$d/bin:$PATH" exec "$d/shell/bin/whiterose-settings" "$@"\n' > "$meta/usr/bin/whiterose-settings"
chmod 755 "$meta/usr/bin/whiterose-settings"
install -Dm644 packaging/files/whiterose-notepad.desktop "$meta/usr/share/applications/whiterose-notepad.desktop"
install -Dm644 packaging/files/whiterose-files.desktop "$meta/usr/share/applications/whiterose-files.desktop"
install -Dm644 packaging/files/whiterose-settings.desktop "$meta/usr/share/applications/whiterose-settings.desktop"
# The apps' icons (Settings, Notepad, Files), every size.
mkdir -p "$meta/usr/share/icons"
cp -r packaging/icons/hicolor "$meta/usr/share/icons/"
# Folders open in Files, in a WhiteRose session only.
install -Dm644 packaging/files/whiterose-mimeapps.list "$meta/etc/xdg/whiterose-mimeapps.list"
install -Dm644 shell/pam/whiterose-lock "$meta/etc/pam.d/whiterose-lock"
install -Dm644 shell/pam/whiterose-fingerprint "$meta/etc/pam.d/whiterose-fingerprint"
install -Dm644 packaging/files/whiterose.desktop "$meta/usr/share/wayland-sessions/whiterose.desktop"
install -Dm644 packaging/files/whiterose-portals.conf "$meta/usr/share/xdg-desktop-portal/whiterose-portals.conf"
# The file chooser portal: Files as every app's Open / Save (whiterosed portal,
# started by D-Bus when the portal first asks).
install -Dm644 packaging/files/whiterose.portal "$meta/usr/share/xdg-desktop-portal/portals/whiterose.portal"
install -Dm644 packaging/files/org.freedesktop.impl.portal.desktop.whiterose.service "$meta/usr/share/dbus-1/services/org.freedesktop.impl.portal.desktop.whiterose.service"
install -Dm644 ARCHITECTURE.md "$meta/usr/share/doc/whiterose/ARCHITECTURE.md"
install -Dm644 packaging/files/whiterose-autostart.sh "$meta/etc/profile.d/whiterose-autostart.sh"
# Leftover wr-* folders in /tmp (often RAM) are cleared after a day unused.
install -Dm644 packaging/files/whiterose-tmpfiles.conf "$meta/usr/lib/tmpfiles.d/whiterose.conf"
install -Dm644 packaging/files/org.whiterose.apps.policy "$meta/usr/share/polkit-1/actions/org.whiterose.apps.policy"
# Updates are only installed when signed with this key (internal/update).
[ -f packaging/release.pub ] && install -Dm644 packaging/release.pub "$meta/usr/share/whiterose/release.pub"
# Where updates come from: project.yaml's repo (the daemon reads it).
mkdir -p "$meta/usr/share/whiterose"
"$work/whiterosed" project project.yaml repo > "$meta/usr/share/whiterose/release-repo"
# Your theme and VS Code extensions, for the app setup.
install -Dm644 profile/custom.zsh-theme "$meta/usr/share/whiterose/profile/custom.zsh-theme"
install -Dm644 profile/vscode-extensions.txt "$meta/usr/share/whiterose/profile/vscode-extensions.txt"

# ---- dependencies: what the binaries link, plus what the shell uses ------------
# (Xwayland: X11 apps; the compositor starts it when the first one connects.)
bins="$v/bin/whiterose-wm $v/bin/quickshell $v/bin/whiterose-notepad $v/bin/whiterose-files"
case "$FORMAT" in
deb)
    # dpkg-shlibdeps maps each linked library to its package.
    mkdir -p "$work/shlib/debian" && : > "$work/shlib/debian/control"
    libs=$(cd "$work/shlib" && dpkg-shlibdeps -O $bins 2>/dev/null | sed 's/^shlibs:Depends=//')
    # QuickShell uses Qt's private API: exactly this Qt.
    qtabi=$(dpkg-query -W -f='${Provides}\n' libqt6core6t64 libqt6qml6 2>/dev/null | tr ',' '\n' | grep -E 'private-abi' | sed 's/^ *//' | paste -sd, -)
    extra="qml6-module-org-kde-syntaxhighlighting, udisks2, qml6-module-qtmultimedia, qml6-module-qtquick-pdf, qml6-module-qtquick-controls, libegl-mesa0, libgl1-mesa-dri, fontconfig, fonts-dejavu-core, fonts-noto-color-emoji, pipewire-audio, pipewire-pulse, qt6-wayland, qml6-module-qtquick, qml6-module-qtquick-controls, qml6-module-qtquick-dialogs, qml6-module-qtquick-shapes, qml6-module-qtquick-window, qml6-module-qtquick-templates, qml6-module-qtqml-workerscript, qml6-module-qtquick-layouts, libqt6svg6, qt6-image-formats-plugins, libegl1, libgles2, pipewire, wireplumber, upower, network-manager, bluez, accountsservice, polkitd, xdg-desktop-portal, xdg-desktop-portal-gtk, dbus-user-session, libglib2.0-bin, adwaita-icon-theme, xdg-utils, procps, findutils, libpam-runtime, xwayland, curl"
    depends="$libs${qtabi:+, $qtabi}, $extra"
    # Fingerprint unlock and sign-in, when there's a reader (installed by
    # default, removable).
    recommends="fprintd, libpam-fprintd"
    ;;
rpm)
    # RPM resolves sonames itself: require each linked library by soname.
    # Only what the binaries link directly (ELF NEEDED): those packages
    # bring their own dependencies (some private, which can't be required).
    libs=$(for b in $bins; do readelf -d "$b"; done | sed -n 's/.*(NEEDED).*\[\(.*\)\]/\1/p' | sort -u | grep -vE '^(ld-linux)' | sed 's/$/()(64bit)/' | paste -sd, -)
    qtver=$(rpm -q --qf '%{VERSION}' qt6-qtbase)
    extra="kf6-syntax-highlighting, udisks2, qt6-qtmultimedia, qt6-qtpdf, mesa-dri-drivers, mesa-libgbm, fontconfig, dejavu-sans-fonts, google-noto-color-emoji-fonts, pipewire-pulseaudio, qt6-qtbase = $qtver, qt6-qtdeclarative = $qtver, qt6-qtwayland, qt6-qtsvg, qt6-qtimageformats, mesa-libEGL, mesa-libGLES, pipewire, wireplumber, upower, NetworkManager, bluez, accountsservice, polkit, xdg-desktop-portal, xdg-desktop-portal-gtk, dbus-tools, glib2, adwaita-cursor-theme, xdg-utils, procps-ng, findutils, pam, xorg-x11-server-Xwayland, curl"
    depends="$libs, $extra"
    recommends="fprintd, fprintd-pam"
    ;;
*) echo "format must be deb or rpm" >&2; exit 2 ;;
esac

# ---- the package -------------------------------------------------------------
if ! command -v nfpm >/dev/null 2>&1; then
    curl -fsSL "https://github.com/goreleaser/nfpm/releases/download/v${NFPM_VERSION}/nfpm_${NFPM_VERSION}_Linux_x86_64.tar.gz" | tar -xz -C /usr/local/bin nfpm
fi
case "$FORMAT" in
deb) scripts="scripts:
  postinstall: $work/src/packaging/files/postinst-deb.sh
  postremove: $work/src/packaging/files/postrm-deb.sh" ;;
*) scripts="" ;;
esac
# A pre-release (0.6.0-alpha.12): the packages' version is 0.6.0~alpha.12,
# which dpkg and rpm both sort before 0.6.0 (a "-" would sort it after).
# Folders and package names keep the version as written.
base="${VERSION%%-*}"
pre=""
case "$VERSION" in *-*) pre="${VERSION#*-}" ;; esac
pkgver="$base${pre:+~$pre}"
yaml_list() { echo "$1" | tr ',' '\n' | sed 's/^ *//; s/ *$//' | grep -v '^$' | sort -u | sed 's/^/  - "/; s/$/"/'; }
cat > "$work/nfpm-core.yaml" <<YAML
name: whiterose-core-$VERSION
version: "$base"
prerelease: "$pre"
version_schema: none
arch: amd64
maintainer: "WhiteRose"
description: |
  WhiteRose $VERSION: the compositor, daemon, shell and apps, in /usr/lib/whiterose/$VERSION.
contents:
  - src: $core/
    dst: /
    type: tree
depends:
$(yaml_list "$depends")
YAML
case "$FORMAT" in
deb) needs="whiterose-core-$VERSION (= $pkgver)" ;;
*) needs="whiterose-core-$VERSION = $pkgver" ;;
esac
cat > "$work/nfpm.yaml" <<YAML
name: whiterose
version: "$base"
prerelease: "$pre"
version_schema: none
arch: amd64
maintainer: "WhiteRose"
description: |
  WhiteRose: a light Wayland desktop (compositor, shell, settings and session daemon).
contents:
  - src: $meta/
    dst: /
    type: tree
depends:
  - "$needs"
recommends:
$(yaml_list "$recommends")
$scripts
YAML
# The version folder's build ID, signed with the packages by `make release`
# (sign-release): an installed WhiteRose checks it's this release, unchanged.
"$work/whiterosed" build-id "$v" > "/out/build-$VERSION-$FORMAT.id"
nfpm package --config "$work/nfpm-core.yaml" --packager "$FORMAT" --target /out/
nfpm package --config "$work/nfpm.yaml" --packager "$FORMAT" --target /out/
# File names without "~" (GitHub renames such assets, and SHA256SUMS
# must name them as they're published).
for f in /out/*~*; do [ -e "$f" ] && mv "$f" "$(echo "$f" | tr '~' '-')"; done
ls -la /out
