# WhiteRose: logging in on the first console starts the desktop (on systems
# without a login screen, like Ubuntu Server). Other consoles stay text-only,
# so there's always a way in if something's wrong. To keep tty1 a console,
# create ~/.config/whiterose/no-autostart.
if [ "$(tty 2>/dev/null)" = "/dev/tty1" ] && [ -z "${WAYLAND_DISPLAY:-}" ] && [ -z "${DISPLAY:-}" ] \
    && [ -z "${SSH_CONNECTION:-}" ] && [ ! -e "$HOME/.config/whiterose/no-autostart" ] \
    && command -v whiterose-session >/dev/null 2>&1; then
    exec whiterose-session
fi
