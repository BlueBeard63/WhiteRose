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

# ---- the file layout ---------------------------------------------------------
install -Dm755 "$work/target/release/whiterose-wm" "$stage/usr/bin/whiterose-wm"
install -Dm755 "$work/whiterosed" "$stage/usr/bin/whiterosed"
install -Dm755 "$work/apps-target/release/whiterose-notepad" "$stage/usr/bin/whiterose-notepad"
install -Dm644 packaging/files/whiterose-notepad.desktop "$stage/usr/share/applications/whiterose-notepad.desktop"
install -Dm755 "$work/apps-target/release/whiterose-files" "$stage/usr/bin/whiterose-files"
install -Dm644 packaging/files/whiterose-files.desktop "$stage/usr/share/applications/whiterose-files.desktop"
# The apps' icons (Settings, Notepad, Files), every size.
mkdir -p "$stage/usr/share/icons"
cp -r packaging/icons/hicolor "$stage/usr/share/icons/"
# Folders open in Files, in a WhiteRose session only.
install -Dm644 packaging/files/whiterose-mimeapps.list "$stage/etc/xdg/whiterose-mimeapps.list"
install -Dm755 "$qs_bin" "$stage/usr/lib/whiterose/quickshell"
ln -s quickshell "$stage/usr/lib/whiterose/qs"
install -Dm755 packaging/files/whiterose-session "$stage/usr/bin/whiterose-session"
mkdir -p "$stage/usr/share/whiterose"
tar -C . --exclude=./shell/tests -cf - shell | tar -C "$stage/usr/share/whiterose" -xf -
ln -s ../share/whiterose/shell/bin/whiterose-settings "$stage/usr/bin/whiterose-settings"
install -Dm644 shell/pam/whiterose-lock "$stage/etc/pam.d/whiterose-lock"
install -Dm644 packaging/files/whiterose.desktop "$stage/usr/share/wayland-sessions/whiterose.desktop"
install -Dm644 packaging/files/whiterose-portals.conf "$stage/usr/share/xdg-desktop-portal/whiterose-portals.conf"
install -Dm644 packaging/files/whiterose-settings.desktop "$stage/usr/share/applications/whiterose-settings.desktop"
install -Dm644 ARCHITECTURE.md "$stage/usr/share/doc/whiterose/ARCHITECTURE.md"
install -Dm644 packaging/files/whiterose-autostart.sh "$stage/etc/profile.d/whiterose-autostart.sh"
install -Dm644 packaging/files/org.whiterose.apps.policy "$stage/usr/share/polkit-1/actions/org.whiterose.apps.policy"
# Your theme and VS Code extensions, for the app setup.
install -Dm644 profile/custom.zsh-theme "$stage/usr/share/whiterose/profile/custom.zsh-theme"
install -Dm644 profile/vscode-extensions.txt "$stage/usr/share/whiterose/profile/vscode-extensions.txt"

# ---- dependencies: what the binaries link, plus what the shell uses ------------
bins="$stage/usr/bin/whiterose-wm $stage/usr/lib/whiterose/quickshell $stage/usr/bin/whiterose-notepad $stage/usr/bin/whiterose-files"
case "$FORMAT" in
deb)
    # dpkg-shlibdeps maps each linked library to its package.
    mkdir -p "$work/shlib/debian" && : > "$work/shlib/debian/control"
    libs=$(cd "$work/shlib" && dpkg-shlibdeps -O $bins 2>/dev/null | sed 's/^shlibs:Depends=//')
    # QuickShell uses Qt's private API: exactly this Qt.
    qtabi=$(dpkg-query -W -f='${Provides}\n' libqt6core6t64 libqt6qml6 2>/dev/null | tr ',' '\n' | grep -E 'private-abi' | sed 's/^ *//' | paste -sd, -)
    extra="qml6-module-org-kde-syntaxhighlighting, udisks2, qml6-module-qtmultimedia, qml6-module-qtquick-pdf, qml6-module-qtquick-controls, libegl-mesa0, libgl1-mesa-dri, fontconfig, fonts-dejavu-core, fonts-noto-color-emoji, pipewire-audio, pipewire-pulse, qt6-wayland, qml6-module-qtquick, qml6-module-qtquick-controls, qml6-module-qtquick-dialogs, qml6-module-qtquick-shapes, qml6-module-qtquick-window, qml6-module-qtquick-templates, qml6-module-qtqml-workerscript, qml6-module-qtquick-layouts, libqt6svg6, qt6-image-formats-plugins, libegl1, libgles2, pipewire, wireplumber, upower, network-manager, bluez, accountsservice, polkitd, xdg-desktop-portal, xdg-desktop-portal-gtk, dbus-user-session, libglib2.0-bin, adwaita-icon-theme, xdg-utils, procps, findutils, libpam-runtime"
    depends="$libs${qtabi:+, $qtabi}, $extra"
    ;;
rpm)
    # RPM resolves sonames itself: require each linked library by soname.
    # Only what the binaries link directly (ELF NEEDED): those packages
    # bring their own dependencies (some private, which can't be required).
    libs=$(for b in $bins; do readelf -d "$b"; done | sed -n 's/.*(NEEDED).*\[\(.*\)\]/\1/p' | sort -u | grep -vE '^(ld-linux)' | sed 's/$/()(64bit)/' | paste -sd, -)
    qtver=$(rpm -q --qf '%{VERSION}' qt6-qtbase)
    extra="kf6-syntax-highlighting, udisks2, qt6-qtmultimedia, qt6-qtpdf, mesa-dri-drivers, mesa-libgbm, fontconfig, dejavu-sans-fonts, google-noto-color-emoji-fonts, pipewire-pulseaudio, qt6-qtbase = $qtver, qt6-qtdeclarative = $qtver, qt6-qtwayland, qt6-qtsvg, qt6-qtimageformats, mesa-libEGL, mesa-libGLES, pipewire, wireplumber, upower, NetworkManager, bluez, accountsservice, polkit, xdg-desktop-portal, xdg-desktop-portal-gtk, dbus-tools, glib2, adwaita-cursor-theme, xdg-utils, procps-ng, findutils, pam"
    depends="$libs, $extra"
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
deps_yaml=$(echo "$depends" | tr ',' '\n' | sed 's/^ *//; s/ *$//' | grep -v '^$' | sort -u | sed 's/^/  - "/; s/$/"/')
cat > "$work/nfpm.yaml" <<YAML
name: whiterose
version: "$VERSION"
arch: amd64
maintainer: "WhiteRose"
description: |
  WhiteRose: a light Wayland desktop (compositor, shell, settings and session daemon).
contents:
  - src: $stage/
    dst: /
    type: tree
depends:
$deps_yaml
$scripts
YAML
nfpm package --config "$work/nfpm.yaml" --packager "$FORMAT" --target /out/
ls -la /out
