# WhiteRose — Architecture Notes

WhiteRose is a custom Wayland desktop. It is not built on niri or Hyprland. Its UI is built in QuickShell, and a Go daemon runs the rest of the system.

## Layers

| Layer | Language | Job |
|---|---|---|
| Compositor (`whiterose-wm`) | Rust + Smithay (decided) | Redrawing only what changed, input, Wayland protocols: `wlr-output-management`, `wlr-layer-shell`, foreign-toplevel, and our own workspace IPC |
| `whiterosed` | Go | Monitor profiles, MPRIS, NetworkManager, BlueZ, UPower, notifications, settings |
| UI | QuickShell (QML) | Bar and panels from the Figma designs. Talks to `whiterosed` over a Unix socket |

### Who owns what

| Area | Owner | Notes |
|---|---|---|
| DRM/KMS, rendering, input, window placement, workspaces, keybindings, screen capture | **Rust** compositor | Must stay fast and small. Holds no settings logic. |
| Monitor profiles, network, Bluetooth, power, audio, notifications, media, updates, module installs, config validation | **Go** `whiterosed` | All "system controls". Talks to system services over D-Bus. |
| Session launcher with health check and rollback | **Go** `whiterose-session` | Tiny and rarely changed. |
| Bar, panels, launcher, switchers, screenshot overlay, Settings window | **QML** (QuickShell) | UI only. No system logic. |
| Settings modules | **QML page + TOML manifest** | Backed by Go providers. Modules never ship their own processes. |

How the parts talk to each other:
- **Compositor ↔ daemon:** standard Wayland protocols wherever one exists (`wlr-output-management`, foreign-toplevel, `ext-image-copy-capture`), plus a small compositor IPC socket for workspaces and actions.
- **Daemon ↔ UI:** a Unix socket with JSON messages.
- **Daemon ↔ system:** D-Bus (NetworkManager, BlueZ, UPower, power-profiles-daemon). Audio needs a different route, because WirePlumber has no D-Bus API.

### Why not Go or C# for the compositor

- A compositor's hot path runs on every frame and every input event.
- In Go, every call into wlroots goes through cgo, which adds overhead.
- wlroots also needs its event loop pinned to one OS thread, which fights the Go scheduler.
- Garbage-collection pauses, even short ones, land in the frame loop.
- Established compositors use Rust + Smithay (niri) or C/Zig + wlroots (Sway, river).

### Why Go for the daemon

- Lower idle memory than C#, and it ships as a single static binary.
- Goroutines suit many D-Bus listeners.
- godbus covers MPRIS, NetworkManager, BlueZ and UPower.
- C# compiled with NativeAOT is workable, but it usually needs more memory to do the same job.

### Battery and efficiency rules

- Wake up only when something happens. Never poll.
- Redraw only the parts of the screen that changed, and nothing at all when nothing changed.
- Let fullscreen apps draw straight to the display, bypassing the compositor.
- Use blur sparingly.
- In QML, run timers and animations (for example the media EQ) only while they are visible and relevant.

### What QuickShell needs from the compositor

QuickShell's workspace modules are written for specific compositors (Hyprland, i3/Sway). Our compositor must provide:

- `wlr-layer-shell`, for the bar and panels
- `wlr-foreign-toplevel-management`, for the window and application switchers
- a workspace IPC of its own, exposed to QML

## Repository layout

```
compositor/   whiterose-wm: Rust + Smithay 0.7
  src/keybind.rs     shortcut parsing, default table, lookup (pure, unit-tested)
  src/workspace.rs   workspaces + focus history for the switchers (pure, unit-tested)
  src/state.rs       compositor state, actions, workspace switching
  src/input.rs       shortcuts first (SUPER tap, PrtSc, ...), then forward to clients
  src/winit.rs       nested backend: redraws only on change (full repaint; the host EGL surface
                     rejects buffer-age queries); dmabuf v4 with render-node feedback
  src/handlers/       protocol handlers: xdg-shell, layer-shell (bar/panels), linux-dmabuf (GPU buffers)
  tests/globals.rs   starts the core headless and connects a real Wayland client
  tests/layer_shell.rs  a bar-like client on a fake 1440x808 output: bar width, reserved space,
                        windows open below it, launcher gets exclusive keyboard focus
daemon/       whiterosed: Go
Makefile      `make check` = gofmt, vet, race, fuzz, cargo fmt, clippy -D warnings, cargo test
              `make run-nested` = WhiteRose in a window on the current desktop
```

When running nested, SUPER is remapped to CTRL+ALT (`--mod ctrl-alt`, the default), because the host desktop keeps SUPER, ALT+TAB and PrtSc for itself.

---

## Monitor configuration: stable IDs

Connector names (`DP-1`, `HDMI-A-2`) describe the port, not the monitor. They change with the cable, the dock and the order monitors are detected. Instead, identify each monitor by its **EDID** identity. The EDID comes from the DRM connector, or from `/sys/class/drm/card*-*/edid`.

### What to hash

Hash only the fields that identify the monitor:

- Manufacturer PNP ID (3 letters), product code, and the 32-bit serial number
- Serial string descriptor (`0xFF`) and monitor name descriptor (`0xFC`), when present
- Week and year of manufacture

Do **not** hash the whole EDID. Some monitors change their EDID when FreeSync, HDR or input settings are toggled.

```
id = "edid:" + sha256(mfr | product | serial | serial_string | week | year)[:12]
```

### Identical monitors

Identical cheap monitors often report serial `0` and no serial string, which gives them the same hash. When that happens:

- Detect the collision.
- Add the port's location to the ID: the DRM `PATH` property for docks and daisy-chained (MST) displays, e.g. `mst:62-1-8`, otherwise the connector name.
- Mark these as **weak IDs** in the config, so the settings UI can ask the user to tell them apart once.

### Config shape

Implemented in `daemon/internal/config`. It's stored as the `displays` setting in the settings database (as JSON, same shape). A `displays.toml` like the example in `daemon/internal/config/testdata/displays.toml` is imported once.

```toml
# Per-monitor preferences follow the monitor everywhere.
[[monitor]]
id    = "edid:3f9a1c2b7e44"
label = "Desk left"          # human name, for the UI only
mode  = "2560x1440@165"      # or "2560x1440", or omit for the preferred mode
scale = 1.25                 # 0.5-4, in steps of 1/120 (Wayland fractional scale)
vrr   = "auto"               # auto | on | off

# Placement depends on which monitors are together, so it lives in profiles.
[[profile]]
name = "desk"

  [[profile.output]]
  id       = "builtin"
  position = [0, 152]

  [[profile.output]]
  id       = "edid:3f9a1c2b7e44"
  position = [1600, 0]
```

- A profile matches when its outputs are **exactly** the connected set. The first match in the file wins, so file order is priority.
- If nothing matches, use an automatic left-to-right layout.
- Unknown keys are rejected (a typo like `sacle` is an error, not silently ignored), and every problem is reported at once.
- `whiterosed monitors` prints the IDs to paste in. `whiterosed check-config` validates the file.

### When a monitor is plugged in or removed

1. The compositor's DRM backend (`compositor/src/drm.rs`) rescans connectors on the udev event, reads each EDID and `PATH` property, and computes the IDs (`compositor/src/monitor_id.rs`, a port of the Go code checked against the same fixture, `testdata/monitor-ids.json`).
2. `display_config::plan` picks the first profile whose monitors are exactly the connected set; otherwise it lays them out left to right, built-in first.
3. Each monitor gets its saved mode, scale, rotation and VRR; disabled monitors are turned off (DPMS). At least one always stays on.

### Where it lives

- Settings are the `displays` key in the settings database. The compositor follows it through `whiterosed` (`compositor/src/daemon_client.rs`) and applies every change at once.
- A layout being tried is the `displays.test` key: `{config, until}`. The compositor shows it until `until`, then goes back by itself and clears the key, so a black screen or a crashed Settings app can't strand you. Keep copies it into `displays`.
- The compositor tells the shell about outputs (IDs, modes, current state, what the backend can change, the trial deadline) in the `outputs` IPC event.
- Plan changed from the first draft: no `wlr-output-management` client in the daemon. The compositor is ours, so it applies the settings itself, which removes a protocol round trip and a moving part.

Note: Hyprland (`desc:`) and niri can already match monitors by make, model and serial. Neither handles duplicate serials or profiles well.

---

## Settings: one module per page

Each settings page is a self-contained **module**. Figma frames: `Settings - Displays` and `Settings - Modules`.

```
modules/displays/
  module.toml     # id, name, icon, category, version, api = 1, providers, requires
  schema.toml     # config keys, types, defaults, validation
  Page.qml        # the UI for the page
```

```toml
# module.toml
id        = "displays"
name      = "Displays"
icon      = "monitor"
category  = "system"
version   = "1.2.0"
api       = 1
providers = ["outputs"]   # backends inside whiterosed this page talks to
requires  = []            # system packages, e.g. ["cups"] for Printers
core      = true          # core modules cannot be disabled
```

Rules that keep RAM low:

- **Modules hold no processes of their own.** The real work lives in a small set of **providers** inside `whiterosed`, for example outputs, audio, network, bluetooth and power. A module only declares which providers it uses, so installing 30 modules does not mean running 30 backends.
- **The settings app loads a page only while it is open.** It loads the page's `Page.qml` with a QML `Loader` and destroys it when you leave the page. Closed pages cost no memory.
- **The settings app is itself on demand.** It is a separate QuickShell window that exits when closed. It is not part of the always-running bar.
- **Config lives in the settings database**, under `module.<name>.…` keys (see Settings database below). `whiterosed` checks values against the module's schema and pushes changes out live, whether they come from the UI or `whiterosed settings set`.
- **Optional modules are not distro packages.** They are just QML plus a manifest. The Modules page downloads, verifies the signature of, and installs them into `~/.local/share/whiterose/modules/`. If a module needs system software (`requires = ["cups"]`), the Modules page installs it through **PackageKit**, which works on both apt and dnf.

---

## Packaging: one file, one command

Goal: no setup scripts and no manual fix-ups.

```
sudo apt install ./whiterose_0.1.0_amd64.deb      # Ubuntu / Debian
sudo dnf install ./whiterose-0.1.0.x86_64.rpm     # Fedora
```

When given a local file, apt and dnf both pull in every dependency the package declares from the distro's repositories. Double-clicking the file in a software center works too.

### What goes inside vs. what is a dependency

| Component | How it ships |
|---|---|
| Compositor | Inside the package. With Rust + Smithay it is one self-contained binary, so it doesn't depend on whichever wlroots version a distro ships (wlroots breaks its API between releases, and Ubuntu LTS lags behind) |
| `whiterosed` | Inside the package. Static Go binary |
| QuickShell | Inside the package. It is not in the standard repos, and older distros may ship a Qt that is too old for it, so ship a private copy under `/usr/lib/whiterose/` (falling back to system Qt only where it is new enough) |
| Core settings modules, configs, icons, fonts (Geist) | Inside the package |
| PipeWire + WirePlumber, NetworkManager, BlueZ, UPower, power-profiles-daemon, polkit, xdg-desktop-portal | Declared as dependencies; apt or dnf installs them |

The package also installs:

- `/usr/share/wayland-sessions/whiterose.desktop`, so "WhiteRose" appears as a choice on the login screen
- systemd **user** units for `whiterosed` and the shell
- default config in `/usr/share/whiterose/`, copied to `~/.config` on first run
- the PolicyKit authentication agent and rules

### Build

- One `nfpm.yaml` builds both the `.deb` and the `.rpm` from the same file list. nFPM is a single Go tool, with no distro build machinery.
- CI produces both files on every tag.
- Later, publish an APT and a DNF repository so updates arrive through the normal `apt upgrade` / `dnf upgrade`.

### Ubuntu vs Fedora RAM

Most of Ubuntu's extra memory use at idle comes from its desktop stack, not the distro base: GNOME Shell, snapd, and services such as apport and tracker. WhiteRose replaces the desktop, so:

- The package must **not** depend on GNOME or `ubuntu-desktop`.
- Recommended Ubuntu base: Ubuntu Server or the minimal install, plus `whiterose.deb`, with snapd removed.
- On that base, idle RAM should be close to Fedora's. Measure it on real hardware before relying on that.

---

## Displays: drag to arrange

Figma frames: `Settings - Displays` and `Settings - Displays (dragging)`.

- The arrangement canvas draws every monitor at its **logical** size (resolution ÷ scale), all multiplied by one shared canvas zoom.
- Dragging a monitor converts the pointer position back to logical pixels. That value is the monitor's `position`.
- **Snapping:** within 14 px (on screen) of another monitor's edge, snap side by side or aligned; the snapped position shows under the tile. Monitors may not overlap, and the layout must stay connected (every monitor touches at least one other).
- After snapping, shift everything so the top-left-most monitor sits at `0,0`. Positions stay positive.
- **Apply previews the whole page** (layout and per-monitor settings) as a `displays.test` trial. A 15 s countdown reverts automatically unless you keep it.
- Confirmed positions are saved against each monitor's **EDID ID** in the active profile. Plugging the same monitors into different ports gives the same layout.
- The X and Y fields in the detail card are the same value, editable precisely. Editing them moves the tile.
- Controls the backend can't honour are greyed out: nested in a window, only scale (and the name) can change.
- Identify shows each monitor's number and name on it for 3 s (`shell/IdentifyOverlay.qml`).
- The logic (snapping, overlap, building the setting) is `settings/modules/displays/Displays.js`, tested by `shell/tests/run.mjs`.

## Appearance

Figma frame: `Settings - Appearance`.

- **Theme** (`appearance.theme`: dark, light, auto). `shell/Theme.qml` holds both palettes and every surface reads it, so a change restyles the bar, panels, island and Settings at once. Text on the accent colour uses `Theme.onAccent` (always dark: every accent is a light pastel).
- **Auto** is light from sunrise to sunset at the weather location (`shell/Sun.js`, the standard sunrise equation), else 07:00–19:00. One timer fires at the next change; nothing polls.
- **Accent** (`appearance.accent`): one of the seven Figma swatches.
- **Font and size** (`appearance.font`, `appearance.size`): the size scales text (`Theme.pt`).
- **Wallpaper** (`wallpaper.current`: `builtin:<name>` or an image path). The shell draws it on a layer-shell background surface per screen (`shell/Background.qml`). The compositor's damage tracking means a still wallpaper costs nothing after the first frame. Built-in gradients need no file. Images are decoded at the screen's size, not the file's. `wallpaper.folders` (default `~/Pictures/Wallpapers`) and `wallpaper.files` are offered, and `wallpaper.rotateMinutes` steps through them.
- **Not built:** "Blur behind panels" from the Figma. The compositor has no blur pass yet, and the row would be a switch that does nothing. Apps' own light/dark preference (the portal's colour scheme) isn't set either. It's shared with GNOME's settings, so changing it here would change GNOME too.

## Windows

- **Minimise:** a minimised window is hidden but kept, and comes back when it's focused (dock, ALT+TAB, the app's launcher entry). It stays hidden across workspace switches.
- **The session's own processes:** the daemon and shell get SIGTERM when the compositor exits, however it exits, so none is left holding the session's sockets.

## Keyboard and shortcuts

Figma frame: `Settings - Keyboard & Shortcuts`.

- **Every shortcut is an action with a stable id** (`compositor/src/keybind.rs`, `CATALOG`: `close-window`, `workspace`, `volume-up`, ...), a name, a group and default keys. The table is the defaults plus the user's changes, rebuilt the moment a setting changes.
- **Settings:** `shortcuts.keys` holds only changes (id → keys; an empty list turns one off); `shortcuts.custom` holds your own `{id, name, command, keys}`. Keys are strings: `SUPER+SHIFT+S`, `PRTSC`, `XF86AudioRaiseVolume`, and `SUPER+1-9` for the nine workspace shortcuts.
- **Keys match by position:** shortcuts use the key's Latin symbol, so they keep working on other layouts. Settings records them the same way (by scan code), so what you press is what's saved.
- **Recording:** Settings sends `pause_shortcuts` so keys reach it instead of triggering shortcuts. Closing Settings (or a crash) un-pauses them. Ctrl+Alt+Fn still works. A combo another shortcut uses shows a warning; Enter takes it over.
- **Layouts:** `keyboard.layouts` (first is the default; UK English, `gb`, out of the box); an empty list means the system's (`/etc/default/keyboard` on Ubuntu, `/etc/X11/xorg.conf.d/00-keyboard.conf` on Fedora), and UK English if the system doesn't say. SUPER+SPACE switches. The compositor sends the layouts' XKB names and the active one in the `keyboard` event. The page lists layouts from XKB's `evdev.lst`.
- **Repeat:** `keyboard.repeatDelay` (150–1000 ms) and `keyboard.repeatRate` (5–80 /s).
- **Media keys** (volume, mic, brightness, play/next/previous) are shortcuts too, so they can be remapped. The shell carries them out (`shell/MediaKeys.qml`: PipeWire, `whiterosed brightness`, MPRIS). On a console session nothing else would handle them.

## Real hardware (DRM backend)

`whiterose-wm --backend drm` (the default when not inside another desktop), or `make run-tty` from a console login.

- **Session:** libseat/logind opens the GPU and input devices. Switching consoles pauses DRM and libinput and resumes them on return. Ctrl+Alt+F1–F12 switch consoles, even when locked.
- **Display:** one GPU (the boot GPU), GBM buffers and a GLES renderer. Smithay's DRM compositor puts the cursor and suitable windows straight on hardware planes.
- **Drawing on demand:** a screen redraws only when something changed, at most once per vblank. When nothing changed, frame callbacks wait one refresh instead of firing at once, so idle clients can't make it spin.
- **Night Light** loads the colour into each screen's gamma table, so it costs no drawing.
- **Cursor:** XCursor theme (`XCURSOR_THEME`, `XCURSOR_SIZE`). Animated cursors show their first frame only, so a busy pointer doesn't redraw the screen continuously.
- **Input:** libinput, with tap-to-click on touchpads. The pointer crosses between screens and stops at outer edges.
- **Scaling:** `wp_fractional_scale_v1` and `wp_viewporter`, so apps draw at exactly 125% or 150% instead of rendering at 200% and being scaled down.
- Only the GPU's own outputs are used; a second GPU (hybrid laptops' discrete card) is ignored for now.

---

## Updates: verify first, keep the old version, roll back automatically

Figma frames: `Settings - Updates` and `Recovery - Update Failed`.

### 1. Download and verification (nothing is touched until this passes)

- Updates come from WhiteRose's signed APT and DNF repositories.
- apt and dnf check the repository signature and every package's checksum **before** installing anything. A partial or corrupted download fails that check and leaves the system untouched.
- The Updates page shows these stages as steps: downloaded → signature → files verified.

### 2. Install alongside, never over

- The **compositor, daemon, shell and core modules** are packaged per version, the same way the kernel is: `whiterose-core-0.5.0` installs into `/usr/lib/whiterose/0.5.0/`.
- `/usr/lib/whiterose/0.4.2/` is left completely alone.
- A small `whiterose` meta-package depends on the newest core package.
- Configure apt and dnf to keep the last 2 versions, as they already do for kernels (dnf `installonlypkgs`; on apt, our own cleanup that keeps the current and previous versions).
- Installing never changes what runs. Only the session launcher decides which version starts.

### 3. Switch on the next session start, with a health check

- `whiterose-session` is a tiny launcher that rarely changes. The login screen's session file points at it.
- It reads `/var/lib/whiterose/state.toml`:
  ```toml
  current  = "0.4.2"
  pending  = "0.5.0"
  attempts = 0
  ```
- With `pending` set, it starts that version and waits for the health check: the compositor, `whiterosed` and the shell must each report "ready" within 20 s, and the session must survive 60 s.
- **Pass:** `pending` becomes `current`, and the old version is kept as the rollback target until the next update.
- **Fail:** increment `attempts`, restore the config snapshot, and start `current` (0.4.2) instead.
- After falling back, the shell shows the recovery prompt: **Try again**, **Stay on 0.4.2**, or **View log**.
- After 2 failed attempts, it stays on the old version automatically and only offers the choice.

### 4. Config safety

- Before an update, snapshot `~/.config/whiterose/` to `~/.local/state/whiterose/snapshots/<version>/`.
- Config files carry a schema version. Migrations only move forward and run on first start of the new version. Rollback restores the snapshot, so the old version never reads a config format it doesn't understand.

### 5. Reboots

WhiteRose updates never need a reboot: restarting the session (or the next login) applies them. Kernel and firmware updates remain the distro's job.

---

## Vendored Smithay

`compositor/vendor/smithay` is Smithay 0.7.0 plus two patches, used through `[patch.crates-io]`.

- **`smithay-layer-unmap-after-destroy.patch`:** destroying a layer surface and then committing its `wl_surface` with no buffer (how QtWayland/QuickShell hides a panel) was rejected with a bogus "width 0 requested" protocol error, which disconnected the whole shell. Test: `tests/layer_shell.rs::unmapping_after_destroying_a_layer_surface_does_not_kill_the_client`.
- **`smithay-session-lock.patch`:**
  - **Unlock disconnect:** the same bug for lock surfaces disconnected the shell on every unlock. Test: `hiding_a_former_lock_surface_after_unlock_does_not_kill_the_client`.
  - **New hooks:** `lock_cancelled` (a lock destroyed before it was confirmed now ends the lock instead of leaving the screen locked forever) and `unlock_requested` (lets the compositor ignore an unlock from a lock that a newer one replaced). Tests: in `tests/session_lock.rs`.
- **Removing them:** drop the vendored copy once upstream ships these fixes.

---

## Clipboard history

Implemented in `compositor/src/clipboard.rs` (pure logic) and `clipboard_capture.rs`; shown in the island's clipboard view.

- **Capture:** every clipboard copy by an app is read by the compositor on a worker thread, with a 3 s timeout and a size cap. It keeps one representation, in this order: file list, image, text.
- **Never stored:** anything a password manager marks as secret (`x-kde-passwordManagerHint`), whitespace-only text, and data over the caps.
- **Storage:** `$XDG_DATA_HOME/whiterose/clipboard/`, containing `history.json` (written atomically) and `images/`. Identical images share one file.
- **Limits:** 50 entries, 256 KB of text per entry, 16 MB per image, 64 MB of images in total. The oldest entries are evicted first. Copying something already in the history moves it to the top.
- **Files:** stored as paths and re-checked on disk every time the history is shown. Missing files are marked; pasting offers only the files that still exist; an entry with none left can't be pasted.
- **Clearing:** Clear removes everything, including the images on disk. Individual entries can be removed too.
- **Drag out:** clipboard entries and screenshots can be dragged from the island into any app (`shell/DragGhost.qml`, `shell/DragData.js`). Files and images go as `text/uri-list` (mail clients attach them; web pages and file pickers upload them) plus their path as `text/plain` (what a text field inserts). Text goes as text; texts over 16 KB aren't sent to the shell and are click-to-paste only. A clipboard image is first copied to `$XDG_RUNTIME_DIR/whiterose/drag/Clipboard image <date>.png`, so it arrives with a readable name; that folder keeps only the 8 newest copies. The compositor draws the drag icon under the pointer (`tests/dnd.rs` drops a file from a panel into a window).
- **Tests:** `WHITEROSE_CLIPBOARD_DIR` overrides the location, and tests always point it at a temporary folder.

## The island

`shell/Island.qml` is one `PanelShape` hanging from the bar. It morphs between every view:

- `idle`
- `dock`
- `clipboard`
- `screenshots`
- `launcher`
- `switcher`, which the compositor drives while ALT+TAB / SUPER+ALT+TAB is held

Each view (`*View.qml`) only reports the size it wants. The island animates its outline to that size, fades the old content out and the new content in once the shape has settled, and keeps the bar window tall until the morph ends.

Scripting: `qs ipc call island set <view>`.

- **Tucking:** when a window sits right under the idle island (a maximized title bar), the compositor sends `{"event":"island","covered":true}`. The island then tucks into the bar, leaving its grip there. Hovering the bar's centre brings it back out, and clicking opens the dock. The compositor re-checks when windows map, move, resize or change workspace (`ipc::island_covered`, tested in `tests/island.rs`).

## Quick settings

`shell/QuickSettingsPanel.qml` hangs from the right end of the bar. It opens from the status cluster or SUPER+S, and closes on Esc or a click elsewhere. Each part comes from the layer that owns it:

| Part | Source |
|---|---|
| Wi-Fi, Bluetooth, volume, battery, power mode | QuickShell services (NetworkManager, BlueZ, PipeWire, UPower, power-profiles-daemon) |
| Brightness, airplane mode | `whiterosed`: `brightness set <percent>` goes through logind's `SetBrightness`; `airplane on\|off` goes through `/dev/rfkill`. Neither needs root. |
| Live brightness/airplane state, lock requests | `whiterosed serve`'s socket: it pushes a `state` event on every change and `{"event":"lock"}` when logind asks (`loginctl lock-session`). It sleeps in the kernel and on D-Bus in between: netlink uevents, rfkill reads, logind signals. |
| Night Light | The compositor (see below). The shell stores the state and turns it off at the next 07:00. |
| Do Not Disturb | Shell state, for the notification server |

Do Not Disturb and Night Light are settings (`quick.doNotDisturb`, `quick.nightLight.until`, `quick.nightLight.temperature`). The power button swaps the toggles for Suspend, Log Out, Restart and Power Off. Suspend locks first.

## Night Light

`compositor/src/night_light.rs` multiplies every pixel by the colour of the chosen temperature, 4000 K by default. In nested mode this is one GL blend pass after each frame (framebuffer × colour), and it runs only while Night Light is on. The DRM backend will load the same colour into the gamma table instead, which costs nothing. Screenshots and window previews render separately, so they stay untinted. `tests/night_light.rs` checks the pass on a real (or Mesa software) GPU.

## Session lock

The compositor implements `ext-session-lock-v1` (`compositor/src/session_lock.rs`). The shell's lock screen (`LockScreen.qml`, `LockView.qml`, `Auth.qml`) locks it. Triggers are the Lock button, SUPER+L, `qs ipc call session lock`, and logind.

- **While locked:**
  - Only the lock surfaces are drawn; an output without one is black.
  - All input goes to the lock surface, and no shortcuts run.
  - Screenshots and window previews are refused.
  - Panels can't take the keyboard.
- **Confirmation:** `locked` is sent to the client only after a frame without the desktop has been shown.
- **Unlocking:** only the locking client can unlock. The password is checked by PAM with `shell/pam/whiterose-lock` (pam_unix); packages install it as `/etc/pam.d/whiterose-lock`.
- **If the shell dies while locked:** the session stays locked. The compositor restarts the shell (`shell_supervisor.rs`, which gives up after 5 exits in a minute). The compositor's `lock` event tells the new shell the session is locked, so it locks again and takes over.

## Notifications

The shell is the session's notification server (`shell/Notifications.qml`, using QuickShell's `NotificationServer`). It supports bodies, actions, images and persistence.

- **The panel** (`NotificationsPanel.qml`) hangs from the bar like Quick Settings and opens from the bell or SUPER+N. It shows everything not yet dismissed, newest first, plus Clear all and a Do Not Disturb toggle.
  - Clicking a card runs the app's default action, or brings the app forward.
  - The app's other actions appear as buttons.
- **Popups** (`NotificationPopups.qml`) show new notifications in the same spot, at most three.
  - They stay for the app's timeout, kept between 3 and 30 s (5 s by default). Hovering pauses the countdown.
  - Critical notifications stay until dismissed and get through Do Not Disturb. Nothing pops up while a panel is open.
  - The bell gets a rose dot until the panel is opened.
- **Rules** (`Notifications.js`) are tested in `shell/tests/run.mjs`.
- **Arrival times:** notifications carry no timestamp, so the shell records when each one arrives.
- **Nested development:** GNOME owns the notification name on the host's session bus, so `make run-nested` gives WhiteRose its own (`dbus-run-session`), like a real login. Apps started inside notify WhiteRose; system services are shared. `notify-send -A` drops actions with this server (a libnotify capability check), so test actions with `gdbus call ... Notify`.

Panels and popups share `SidePanel.qml`: the shape, the grow and shrink animation, and Esc or a click elsewhere to close.

## Calendar and weather

The calendar panel (`shell/CalendarPanel.qml`) opens from the clock. It shows the time and date, the weather, a month grid (Monday first) with a dot on days that have events, and the selected day's events. All data comes from `whiterosed`:

- **Calendars** (`daemon/internal/calendar`, `internal/ical`). Read-only.
  - **Sources:** the `calendar.sources` setting: iCal links (Google, Outlook and iCloud "secret address" links, any `.ics` URL) and `.ics` files, plus files dropped into `~/.local/share/whiterose/calendars/`. Links are kept in the keyring, never in the settings. They're changed with their own requests (`whiterosed calendar add|remove|list`), not a plain `set`.
  - **Fetching:** links go into `~/.cache/whiterose/calendars/` every 15 minutes, using ETags, a size cap, and file names hashed so the secret link isn't exposed. The panel reads the cache: it's instant and works offline.
  - **Reader:** the iCalendar reader handles time zones (including Outlook's Windows zone names), all-day and multi-day events, DURATION, RRULE (daily, weekly, monthly, yearly with INTERVAL, COUNT, UNTIL, BYDAY, BYMONTHDAY), EXDATE, moved occurrences (RECURRENCE-ID) and cancellations. It's fuzzed in `make check`.
  - **Commands:** `whiterosed calendar list | add NAME URL|FILE [#COLOUR] | remove N | events FROM TO`.
- **Weather** (`internal/weather`).
  - **Place:** the `weather.city` setting (or `weather.latitude` and `weather.longitude`), plus `weather.units`. Location is never detected. Changing them refreshes at once.
  - **Data:** Open-Meteo (no account), refreshed every 30 minutes. The city is looked up once and cached. Readings are cached in `~/.cache/whiterose/weather.json`.
- **Updates to the shell:** `whiterosed serve` runs both refreshes and adds `weather` and a `calendars` version to its state events. The shell re-reads events when the version changes.

## Settings database

All user settings live in one SQLite database, `~/.local/share/whiterose/settings.db`. Both the folder and the file are readable only by the user. It's owned by `whiterosed serve`, which the compositor starts and restarts (`--daemon`), one per session, on `$WHITEROSE_DAEMON_SOCKET`.

- **Keys and values:** each setting is a key holding a JSON value (`dock.pinned`, `launcher.*`, `quick.*`, `weather.*`, `calendar.sources`, `displays`, `keyboard.*`, `shortcuts.*`).
  - Known keys have a default and a type check (`daemon/internal/store/keys.go`).
  - Settings modules keep theirs under `module.<name>.`.
  - Bad values are refused with a reason.
  - The schema is versioned, and a database from a newer WhiteRose is left untouched.
- **Live:** clients get every value on connect and every change as it happens (`daemon/internal/server`). The shell reaches it through `shell/Daemon.qml`; the CLI is `whiterosed settings list|get|set|reset`.
- **Secrets** (calendar links) go into the desktop keyring through the Secret Service (GNOME Keyring, KWallet), but only one already running and unlocked; the daemon never starts one or causes a prompt. Without one (nested development, a session with no keyring), they're kept in the database's own table and moved into the keyring once there is one.
- **Import:** at start, any old config file found (`dock.json`, `launcher.json`, `quick.json`, `calendars.toml`, `weather.toml`, `displays.toml`) is imported and renamed `*.migrated`. A file that can't be read is left alone and reported.
- **Not settings:** clipboard history (the compositor's data, with images) stays in `~/.local/share/whiterose/clipboard/`.
- **Tests:** store, validation, secrets (with a fake keyring), import, the socket protocol with real clients, and the CLI against an in-process daemon.

## Settings app

`shell/settings.qml` is a separate QuickShell window, started on demand by `shell/bin/whiterose-settings [page]` (the Quick Settings gear and "All settings"). Running it again only switches the open window to the page: the launcher finds the running window by its command line and tells it over QuickShell IPC (QuickShell's own lookup by config path isn't reliable). It exits when closed. It draws its own frameless, rounded window, because WhiteRose has no server-side decorations.

- **Pages are modules:** each is a folder with `module.json` (id, name, icon, category, order, version, core, keywords) and `Page.qml`. Built-in pages live in `shell/settings/modules/`; installed ones go in `~/.local/share/whiterose/modules/`.
  - Only the open page is loaded.
  - The Modules page turns optional pages off (`settings.modules.disabled`).
  - Search matches names, descriptions and keywords.
- **Shared components** live in `shell/settings/`: cards, rows, toggles, dropdowns, segmented control, fields, sliders and dialogs.
- **Pages in this first set**, each backed by the real service:

| Page | Backed by |
|---|---|
| Network | QuickShell Networking for the list, connect and forget; `whiterosed` (`internal/netconf`) for profile details and editing: security, password, MAC privacy, frame protection, IPv4/IPv6, DNS, metered, autoconnect. Edits keep the stored password and re-apply the profile if it's in use. |
| Bluetooth | QuickShell Bluetooth: adapter, pair, connect, forget. Searches only while the page is open. |
| Sound | QuickShell PipeWire: default output and input, volumes, per-app streams. |
| Power & Battery | UPower and power profiles; `whiterosed` (`internal/power`) for cycles, the 24 h charge history and the charge limit (shown only where the battery supports one); `bar.batteryPercent`. |
| Dock | `dock.pinned`: order, remove, add from installed apps. |
| Notifications | Do Not Disturb, popup duration, per-app mute (the shell records which apps notify). |
| Calendar & Weather | `whiterosed` calendar add/remove (links to the keyring) and the `weather.*` settings. |
| Keyboard & Shortcuts | Compositor `shortcuts` and `keyboard` events; `keyboard.*` and `shortcuts.*` settings. |
| Appearance | `appearance.*` and `wallpaper.*`; the wallpaper list from `find` in the folders. |
| Displays | Compositor `outputs` event; Apply writes the `displays.test` trial, Keep writes `displays`. |
| Modules, About | The module list and memory use; hardware and OS from `/proc`, `/sys` and `/etc/os-release`. |

- **Pages not built yet** don't appear in the sidebar: Top Bar, Updates, Users, Privacy. They wait on their backends: the update system.
