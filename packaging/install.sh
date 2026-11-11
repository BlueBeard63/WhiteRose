#!/bin/sh
# Installs WhiteRose from a GitHub release: both packages (whiterose and
# its whiterose-core-<version>), checked against the release's signed
# SHA256SUMS before anything is installed.
#
#   curl -fsSL https://github.com/@REPO@/releases/latest/download/install.sh | sh
#
# Or with the packages already downloaded, run it in their folder (it uses
# them instead of downloading). `make release` fills in the version, the
# repository and the release key; this copy installs @VERSION@.
#
#   install.sh --check   download and check everything, install nothing
set -eu
check_only=false
case "${1:-}" in
--check) check_only=true ;;
"") ;;
*) echo "usage: install.sh [--check]" >&2; exit 2 ;;
esac

VERSION="@VERSION@"
REPO="@REPO@"
PUBKEY="@PUBKEY@"

# ---- looking nice ----------------------------------------------------------------
if [ -t 1 ]; then
    rose=$(printf '\033[38;2;232;163;174m'); dim=$(printf '\033[2m'); bold=$(printf '\033[1m')
    red=$(printf '\033[38;2;229;83;75m'); off=$(printf '\033[0m')
else
    rose=""; dim=""; bold=""; red=""; off=""
fi
step() { printf '%s●%s %s\n' "$rose" "$off" "$1"; }
note() { printf '  %s%s%s\n' "$dim" "$1" "$off"; }
fail() { printf '%s✗ %s%s\n' "$red" "$1" "$off" >&2; exit 1; }

printf '\n  %sWhiteRose%s %s\n\n' "$bold$rose" "$off" "$VERSION"

# ---- which kind of system ---------------------------------------------------------
if command -v apt-get >/dev/null 2>&1 && command -v dpkg >/dev/null 2>&1; then
    kind=deb
    core="whiterose-core-${VERSION}_${VERSION}_amd64.deb"
    meta="whiterose_${VERSION}_amd64.deb"
elif command -v dnf >/dev/null 2>&1 && command -v rpm >/dev/null 2>&1; then
    kind=rpm
    core="whiterose-core-${VERSION}-${VERSION}-1.x86_64.rpm"
    meta="whiterose-${VERSION}-1.x86_64.rpm"
else
    fail "WhiteRose installs on Ubuntu (apt) or Fedora (dnf); this system has neither."
fi
[ "$(uname -m)" = x86_64 ] || fail "WhiteRose is built for x86_64; this is $(uname -m)."
for tool in sha256sum openssl base64; do
    command -v "$tool" >/dev/null 2>&1 || fail "$tool is needed to check the download."
done
step "Installing WhiteRose $VERSION on $( . /etc/os-release 2>/dev/null && echo "${PRETTY_NAME:-this system}" || echo "this system")"

# ---- the files: here already, or downloaded ---------------------------------------
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
here=$(pwd)
if [ -f "$here/$core" ] && [ -f "$here/$meta" ] && [ -f "$here/SHA256SUMS" ] && [ -f "$here/SHA256SUMS.sig" ]; then
    step "Using the packages in $here"
    for f in "$core" "$meta" SHA256SUMS SHA256SUMS.sig; do cp "$here/$f" "$work/"; done
else
    command -v curl >/dev/null 2>&1 || fail "curl is needed to download WhiteRose."
    base="https://github.com/$REPO/releases/download/v$VERSION"
    step "Downloading"
    for f in SHA256SUMS SHA256SUMS.sig "$core" "$meta"; do
        note "$f"
        curl -fsSL --retry 3 -o "$work/$f" "$base/$f" || fail "couldn't download $f from $base"
    done
fi

# ---- checked before anything is installed -----------------------------------------
step "Checking the signature"
# The release key is a raw Ed25519 key: openssl wants it wrapped as DER.
printf '\060\052\060\005\006\003\053\145\160\003\041\000' > "$work/key.der"
printf '%s' "$PUBKEY" | base64 -d >> "$work/key.der" 2>/dev/null || fail "the release key in this script is damaged."
base64 -d < "$work/SHA256SUMS.sig" > "$work/sig.bin" 2>/dev/null || fail "SHA256SUMS.sig is damaged."
openssl pkeyutl -verify -pubin -inkey "$work/key.der" -keyform DER -rawin \
    -in "$work/SHA256SUMS" -sigfile "$work/sig.bin" >/dev/null 2>&1 \
    || fail "SHA256SUMS isn't signed by the WhiteRose release key: not installing."
step "Checking the packages"
awk -v a="$core" -v b="$meta" '$2 == a || $2 == b' "$work/SHA256SUMS" > "$work/want"
[ "$(wc -l < "$work/want")" -eq 2 ] || fail "SHA256SUMS doesn't list both packages."
(cd "$work" && sha256sum -c --quiet want) || fail "a package doesn't match its checksum: not installing."

if $check_only; then
    printf '\n%s✓%s Both packages are signed and intact (nothing installed).\n' "$rose" "$off"
    exit 0
fi

# ---- installing ---------------------------------------------------------------------
as_root=""
[ "$(id -u)" -eq 0 ] || as_root=sudo
step "Installing (this asks for your password)"
if [ "$kind" = deb ]; then
    $as_root apt-get install -y "$work/$core" "$work/$meta"
else
    $as_root dnf install -y "$work/$core" "$work/$meta"
fi

printf '\n%s✓%s WhiteRose %s is installed.\n' "$rose" "$off" "$VERSION"
note "Log out and pick WhiteRose at the login screen. Updates come through Settings › Updates."
