# XWayland: X11 apps in WhiteRose

Status: in progress (2026-11-12): steps 1 to 3 done, branch `v0.3/xwayland` (off `v0.3/alpha-fixes`,
which touches the same compositor files).

## Why

The compositor speaks only Wayland, so an X11-only app has nowhere to draw:
Spotify's Linux app, Steam and most games, older tools, anything Electron or
CEF that isn't told to use Wayland. Found when Spotify wouldn't open (and so
the Music module had nothing to show).

## What we have

- Smithay (vendored, patched) has XWayland support behind its `xwayland`
  feature: `XWayland::spawn` starts the server (`-rootless -terminate`, its
  own listening sockets), `X11Wm` is the window manager side, `XwmHandler`
  the callbacks, and `desktop::Window` can hold an `X11Surface` as well as a
  Wayland toplevel.
- Ubuntu and Fedora ship `Xwayland`; the packages need to depend on it.
- About 40 places in the compositor reach for `window.toplevel()` (a Wayland
  xdg toplevel) to read the app id and title, set sizes and states (tiled,
  maximized, fullscreen, activated), close, read size limits, or find the
  client's pid. An X11 window has none of that, so each needs another way.

## Decisions

- **One way to talk to a window, either kind** (`win.rs`): ids, size and
  states, close, size limits, whether it's a dialog, pid, its surface.
  Wayland windows behave exactly as now; X11 ones map onto `X11Surface`
  (WM_CLASS is the app id, `_NET_WM_NAME` the title, `configure` sets size
  and place together).
- **X11 windows are windows like any other:** workspaces, tiling, groups,
  snapping, rules, scratchpads, the switchers, the dock and foreign-toplevel
  all see them. Override-redirect ones (menus, tooltips) aren't managed:
  drawn where they ask, on top, never in a workspace.
- **Place is X11's business too:** an X11 window must be told where it is
  (its own coordinates), so whenever the compositor places one it configures
  it with its rectangle, not just its size.
- **DISPLAY for everyone:** apps the compositor starts, the daemon, and
  D-Bus-activated services (`dbus-update-activation-environment DISPLAY`).
- **Efficiency:** start Xwayland the first time an X11 app connects, not at
  login, and let it exit when the last one closes (`-terminate`): an idle
  desktop with no X11 apps runs no X server. This needs Smithay to accept
  listening sockets we opened (a small patch in the vendored copy).
- **Clipboard and drag and drop between X11 and Wayland apps** through the
  X11 WM's selection support, so copy-paste works both ways and the
  clipboard history sees X11 copies.
- **Scale:** X11 apps are drawn at scale 1 and scaled up (blurry at 150%)
  in the first version; sharp X11 apps at fractional scale are a later step.

## Steps (a commit each)

1. **`win.rs`**: the one way to talk to a window; every `toplevel()` site
   goes through it. No change in behaviour (Wayland only so far).
2. **Xwayland runs**: the `xwayland` feature, the server started with the
   compositor (winit and DRM), `X11Wm`, DISPLAY exported; X11 windows map,
   draw, take focus and input, move, resize and close. Override-redirect
   windows drawn unmanaged.
3. **Everything else sees them**: tiling, maximize and fullscreen, snapping,
   groups, rules, scratchpads, switchers, dock, IPC ids.
4. **Clipboard and drag and drop** between X11 and Wayland.
5. **Started on demand**: the vendored Smithay patch, Xwayland launched on
   the first X11 connection.
6. **Packaging**: depend on Xwayland (`xwayland` / `xorg-x11-server-Xwayland`).

Tests: compositor integration tests with an X11 client (x11rb) against a
real Xwayland in the headless backend (skipped where Xwayland isn't
installed), plus a UI test opening an X11 app.
