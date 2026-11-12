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
| Session launcher with health check and rollback | **Go** `whiterose-launch` (run by `whiterose-session`) | Tiny and rarely changed. |
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
  src/layout/        window layouts (pure, unit-tested): the Layout enum and which one applies,
                     gaps, the dwindle tree, master, drop zones, tab groups
  src/tiling.rs      tiles per (workspace, output): retile(), floating geometry, mouse drops, the
                     tiling keys, the windows.* settings
  src/tab_strip.rs   a grouped tile's tab strip, drawn with fontdue in the bundled Geist
  src/grabs/         pointer grabs: move, resize, and tile_grab.rs (drag a tile, move a split)
  src/state.rs       compositor state, actions, workspace switching
  src/input.rs       shortcuts first (SUPER tap, PrtSc, ...), then forward to clients
  src/winit.rs       nested backend: redraws only on change (full repaint; the host EGL surface
                     rejects buffer-age queries); dmabuf v4 with render-node feedback
  src/headless.rs    headless backend (`--backend headless`): one virtual 1280x800 output, nothing
                     drawn, frame callbacks at most every 16 ms; input only through the debug
                     pointer and keys. For the UI tests in CI
  src/handlers/       protocol handlers: xdg-shell, layer-shell (bar/panels), linux-dmabuf (GPU buffers)
  tests/globals.rs   starts the core headless and connects a real Wayland client
  tests/layer_shell.rs  a bar-like client on a fake 1440x808 output: bar width, reserved space,
                        windows open below it, launcher gets exclusive keyboard focus
  tests/tiling.rs, tile_mouse.rs, tile_groups.rs, tile_keys.rs  tiling with real clients: layouts
                        and settings, dragging and resizing, tab groups, the keys
daemon/       whiterosed: Go
  uitest/            click-and-check UI tests (`make ui-test`): internal/uitest starts a nested
                     WhiteRose in a session of its own (HOME, D-Bus, accessibility bus, display),
                     finds things by accessible name and role (AT-SPI; the kit's components and
                     Files' rows and tabs carry them), clicks and types with the real pointer and
                     keyboard through the compositor, and reads back what shows. The shell (bar,
                     island, panels) and Settings too: layer windows are titled with their
                     namespace (shell/LayerTitle.qml) and placed through the compositor's
                     test-only `{"debug_layers":true}`; settings can be seeded and read back. Nested in the
                     current desktop, or headless (apps draw in software) when there is none or
                     WR_UITEST_HEADLESS=1: the `ui` CI job. `make shots` (shots_test.go) uses
                     the same sessions to save screenshots for review to build/shots: the bar,
                     Quick Settings, the zone editor on screen, and every Settings page as a tile
Makefile      `make check` = gofmt, vet, race, fuzz, cargo fmt, clippy -D warnings, cargo test
              `make run-nested` = WhiteRose in a window on the current desktop
```

When running nested, the window asks the host desktop to pass its shortcuts through, so SUPER works as on real hardware. Where the host can't (X11, or the permission was refused), `--mod ctrl-alt` remaps SUPER to CTRL+ALT.

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

## Packaging: how it's built (`make package`)

- Each package is built in a container of its distro (`packaging/Dockerfile.ubuntu`, `packaging/Dockerfile.fedora`, `packaging/build.sh`), so it links that distro's own libraries. Output goes to `build/packages/`.
- QuickShell is built from source, pinned to the revision the shell is written against, and shipped privately as `/usr/lib/whiterose/quickshell`. It uses Qt's private API, so the package requires exactly the Qt it was built with: `qt6-*-private-abi (= x.y.z)` on Ubuntu, `qt6-qtbase = x.y.z` on Fedora. A distro Qt update waits for a rebuilt WhiteRose instead of breaking the shell.
- Dependencies come from what the binaries link (dpkg-shlibdeps; sonames for rpm), plus what a bare install lacks: Mesa's drivers, fonts, PipeWire with Pulse compatibility, NetworkManager, BlueZ, UPower, AccountsService, polkit and the portals.
- **Starting it:** the login screen's "WhiteRose" session (`/usr/share/wayland-sessions`). Without a login screen (Ubuntu Server), logging in on tty1 starts it (`/etc/profile.d/whiterose-autostart.sh`; zsh users get the same line in `~/.zprofile`). Other consoles stay text.
- **Ubuntu Server networking:** the package sets netplan's renderer to NetworkManager (`/etc/netplan/90-whiterose-network-manager.yaml`, from the next boot; removed on purge) so Wi-Fi and the Network page work.
- **App setup** (Settings > Apps, offered on first login): `daemon/internal/apps`. Each app comes from the distro's packages, the maker's repository (VS Code, Signal on Ubuntu) or a COPR (Ghostty on Fedora), else Flathub. No snaps. The root half runs as `pkexec whiterosed apps-root ID...` (policy `org.whiterose.apps.install`). It only accepts catalogue ids, and every URL and command is fixed in the catalogue. Then, as you: Oh My Zsh with your theme, and your VS Code extensions (`profile/`).

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

## Top bar

- `bar.*` settings choose what the bar shows: workspaces (`bar.workspaceStyle`: dots or numbers), now playing, the network, Bluetooth and volume icons, the battery percentage, the bell, the date, 12- or 24-hour time, and seconds. `bar.screens` puts the bar on every screen or only the main one.
- Seconds are the only option with a cost: the clock then wakes every second instead of every minute.
- **Icons:** the bar's are solid glyphs on a 16px grid (`shell/Glyph.qml`; Figma: `Bar Icons — 16px`), drawn 1:1 and placed on whole pixels (`Trigger` centres its row on whole pixels too), so they stay sharp. Lucide's 24px outlines, shrunk to 13–16px, landed between pixels and went soft. Weak Wi-Fi draws the bands it doesn't reach dim, and Do Not Disturb shows a moon. Elsewhere (Settings, panels) icons stay Lucide outlines (`Icon.qml`).

## Screenshot tool

Figma frame: `Screenshot - Region (SUPER + SHIFT + S)`.

- **The toolbar takes the island's place**, drawn in the overlay itself so it sits above the dim. The overlay (`shell/ScreenshotOverlay.qml`, one per screen, layer-shell Overlay) dims everything but the selection. The key-hint row in the Figma is left out: no key hints on island views.
- **Region:** drag to select, drag inside to move, eight handles resize, Shift keeps it square (`shell/Shot.js`, tested). **Window:** point and click; the window is raised before the capture. **Screen:** the screen under the pointer.
- **Delay** (3/5/10 s) and **Pointer** (drawn into the capture; only where the compositor draws the pointer, so greyed out nested). **Copy** puts it on the clipboard only; **Save** also writes `~/Pictures/Screenshots`. Enter saves, Ctrl+C copies, Esc cancels.
- **The compositor captures** (`capture` request: region, pointer, save). The tool hides itself first, so it's never in the shot. A region is cropped from the frame of the output holding its centre.

## Users and permissions

- **The shell is the session's polkit agent** (`shell/PolkitPrompt.qml`, QuickShell's PolkitAgent). When anything needs permission, an app or Settings, a card asks for the password over a dimmed screen. A wrong password shakes it and lets you retry. When the system's PAM asks for a fingerprint first (fprintd enabled for `polkit-1`), the card shows the reader listening, and the password field takes over when PAM moves on. Nested inside another desktop, that desktop stays the agent (polkit allows one per session).
- **Accounts** (`daemon/internal/accounts`): AccountsService over D-Bus. Listing, name, picture, administrator rights, password, adding and removing people. Calls allow interactive authorization, so polkit asks through the prompt. Passwords are hashed in the daemon (SHA-512 crypt, checked against the specification's test vectors) and never sent in the clear. You can't remove your own administrator rights or your own account.
- **Fingerprints** (Settings › Users › Fingerprints; `daemon/internal/fingerprint`, tested against a fake fprintd on a private bus): fprintd over D-Bus. Lists the reader and your enrolled fingers, adds one (each touch is a `fingerprint` event; the ring fills; it stops after 2 min untouched or when the dialog closes), removes one. fprintd asks for your password before adding or removing (its own `auth_self_keep` rule), through the shell's prompt. The lock screen uses any enrolled finger by itself. **Sign-in, sudo and admin prompts** are the system's PAM, for everyone on the computer: the toggle runs `whiterosed fingerprint-root on|off` through pkexec (polkit action `org.whiterose.fingerprint.login`, admin password), which runs `pam-auth-update --enable/--remove fprintd` (Ubuntu) or `authselect enable-feature/disable-feature with-fingerprint` (Fedora). Its state is read from `/etc/pam.d/common-auth` or `system-auth`.
- **Root helpers** run as `/usr/bin/whiterosed <command>-root` under pkexec when installed (the path the polkit actions name, so their own message shows), else the running binary (a build being worked on).
- **Slow requests don't block:** whiterosed answers the optional ops (users, battery, network profiles) on their own goroutines, so a request waiting on a password doesn't hold up the shell's settings traffic. Settings requests stay in order.

## Privacy: idle and history

- **Idle** (`compositor/src/idle.rs`): `privacy.lockAfter` and `privacy.screenOffAfter` (minutes; 0 is never). Input only stamps the time. One timer wakes at the next deadline: it asks the shell to lock, or turns every screen off (DPMS), and the next input turns them back on. Only on real hardware: nested, the host desktop handles idle.
- **Staying awake:** a window on screen holding `zwp_idle_inhibit_v1` (a video, a call) holds both off.
- **Lock before sleep** (`privacy.lockOnSuspend`): whiterosed holds a logind delay inhibitor. On PrepareForSleep it tells the shell to lock and lets the system sleep a second later.
- **History:** `privacy.clipboardHistory` off stops recording (copy and paste still work). `privacy.recentFiles` keeps recent files out of search. Settings can clear the clipboard history and the launcher's remembered commands, sums and app counts.

## WhiteRose's own apps (`apps/`)

Each app is Qt Quick for the UI and Rust for the work, bridged with cxx-qt. It is one binary with its QML and fonts compiled in. The apps share a Cargo workspace.

- **The kit** (`apps/kit`, `import WhiteRose.Kit`):
  - `Desktop`: a QML singleton for the session's settings, following whiterosed live, so apps restyle the moment Appearance changes;
  - `Theme`: the shell's tokens, with Auto using the same sunrise maths as the shell;
  - the Settings app's controls;
  - `AppWindow`: rounded, with the desktop's Windows-style title buttons, a title-bar toolbar slot and edge resizing;
  - `Menu`.
- **Rule for QML here:** write properties qualified (`root.x = …`, never a bare `x = …` in a function). The QML is compiled ahead of time, and a bare write silently goes nowhere.
- **App icons:** `packaging/icons/hicolor/<size>/apps/whiterose-{settings,notepad,files}.png` (16 to 512 px), installed to `/usr/share/icons/hicolor` by the packages. `make dev-apps` puts the three launcher entries (pointing at this checkout's builds) and the icons in `~/.local` for running from source; `make dev-apps-remove` takes them out.
- **Notepad** (`apps/notepad`, `whiterose-notepad [FILE…]`):
  - **Tabs:** the scratch note first, then files. The scratch note always stays and saves itself as you type.
  - **Editing:** find and replace, go to line, wrap, line numbers placed at each line's real position (only the visible ones), and text size.
  - **Highlighting:** KDE's syntax-highlighting library when installed (a package dependency; plain text without it).
  - **Saving:** a new file is written and renamed over the old, keeping permissions and line endings. Binary, non-UTF-8 and >20 MiB files are refused.
  - **Changes on disk:** a file changed underneath you reloads if you haven't edited it, else asks.
  - **Session:** the open files come back next time (`$XDG_DATA_HOME/whiterose/notepad`).
- **Settings sub-pages:** a module's `module.json` can list `pages` ({id, name, icon, file, keywords}); they show indented under it in the sidebar while it's open (or match a search), and open as `module/page` (`whiterose-settings files/search`, the IPC `open`).
- **Files** (`apps/files`, `whiterose-files [FOLDER…]`; the feature checklist is `apps/files/FEATURES.md`):
  - **Browsing:** tabs, breadcrumbs (`… > Folder` when long; click for the path as text), grid, list and columns, sidebar places, bookmarks (GTK's file, shared with other file managers) and drives.
  - **Tabs and windows:** drag to reorder; out of the bar a tab becomes a real drag (`application/x-whiterose-tab`: its snapshot, the process's token, the window and an id). Any Files window's title bar takes it, in this process or another; dropped anywhere else it gets a window of its own. The taking window leaves `$XDG_RUNTIME_DIR/whiterose/files-tabs/<id>` for the giving one to find when its drag ends, since which drop action a drag reports varies between processes and compositors. Duplicate; reopen closed.
  - **Split tabs** (`TabView.qml`): F3 splits a tab into two folders, side by side or one above the other (a SplitView). The side last used is the window's `current`, so the toolbar and keys follow it; F5 / F6 copy / move the selection to the other side. The session keeps the second side, the direction and the side in use. Started without folders, Files owns the session (`$XDG_STATE_HOME/whiterose/files/session.json`) and every window's tabs come back; started on a folder, it leaves the session alone.
  - **File operations** (`src/ops.rs`, run by the `Ops` singleton in `src/jobs.rs`): copy, move, trash, delete on a thread per job, copies and moves one at a time. Copies write `.name.wrpart` and rename it into place, keeping modes and times. A clash pauses the job and asks (replace or merge, keep both, skip, or the same for the rest). Moves rename when they can, else copy then remove the original only if everything copied.
  - **Trash:** the freedesktop.org layout (`src/trash.rs`), so GNOME and KDE see the same Trash: home Trash for the home drive, the drive's `.Trash/$uid` or `.Trash-$uid` elsewhere. `trash:` (also `trash:///` from `xdg-open`) shows every Trash at once, by original name, with where each came from and when. Restore puts it back (remaking its folder; numbered if the name's taken); deleting there ("purge") removes the `.trashinfo` too.
  - **Properties:** `Fs.info` for one item; sizes of folders and of several items are counted on a thread (`Ops.measure`) and shown as they grow. Permission switches `chmod` straight away, only for your own files.
  - **Drag and drop:** a drag carries the selection as `text/uri-list` (and the paths as text), so any app takes it; drops come the same way from any app. Targets: folders in every view, a folder's empty space, sidebar places (the Trash trashes), breadcrumbs and tabs (held over, a tab opens). `Drops.plan` decides: same drive moves, another copies, Ctrl copies, Shift moves (modifiers read at the drop, `QGuiApplication::queryKeyboardModifiers`); never onto or into itself. The source app is always told "copied", so it never deletes originals behind our move. Drags start by hand past 10 px (`DragSource.qml`), not with `MouseArea.drag.target`: with that, Qt crashed when the dragged item went away mid-drag.
  - **Search** (`src/search.rs`, run by the `Search` singleton in `src/finder.rs`): typing in a folder filters it; Ctrl+F searches. Names must hold every word (any order), ranked by where the words fall and by recency; `*.pdf` / `ext:pdf` and type, size and date filters narrow it. "This folder" walks the tree live (always exact); "Home" and "Everywhere" read the index: `$XDG_CACHE_HOME/whiterose/files/index` (0600), folders before their contents and each tree in one stretch so a search is one pass through a memory-mapped file. It's rebuilt by `whiterose-files --index` (`src/indexer.rs`), a process of its own at nice 19 and idle I/O priority (so closing Files doesn't stop it; a lock keeps it to one), when missing or over an hour old, or on request. It reads what to index and whether to seal it from whiterosed, and makes nothing without it. While it runs, `index.progress` beside the index holds `{pid, started, count, expected}` (`expected`: the last index's size); Files polls it for the bar, the percentage and the time left. Hidden folders, `node_modules`, `__pycache__`, `site-packages` and `~/go/pkg` are left out; hits gone since are dropped.
  - **Previews** (`Preview.qml`): pictures decoded at the size shown, the start of text (`Fs.readText`: up to 16 KB in the panel, 1 MB in quick look; not text if it has NULs or isn't UTF-8), and anything else as its type's large icon. Video, audio (`MediaPreview.qml`, QtMultimedia) and PDF (`PdfPreview.qml`, QtQuick.Pdf) are plain resources loaded only when needed, not compiled ahead, so Files builds and runs without those modules (showing icons; a missing module isn't retried). The packages depend on them. Grid thumbnails are the photos themselves, decoded at icon size, only for visible items.
  - **Open in a terminal / editor** (`src/launch.rs`): the `apps.terminal` and `apps.editor` settings (a program's name, chosen in Settings › Apps, which lists only installed ones), else the first installed from a fixed list; each terminal's own way of starting in a folder. Started in their own session, so they outlive Files. A folder inside a git repository also offers the repository.
  - **Folder sizes:** list view asks `Ops.sizeFolders` for the folders shown; one idle-priority thread works through them, dropping the queue when asked again (moving folder) and reusing sizes from the last minute. Not for `/`.
  - **Storage** (`drives:`): every mounted drive as a card with how full it is, refreshed while shown.
  - **Shortcuts:** one table in `Main.qml` (id, label, default); `files.keys` in whiterosed holds only the changed ones, so every window follows a change at once.
  - **Folder colours and icons** (`src/looks.rs`, `$XDG_STATE_HOME/whiterose/files/looks.json`): a colour from a fixed palette, a symbol (kit icon names) or a picture, per folder path. `FolderIcon.qml` draws the folder itself (two shapes and the symbol), so it's sharp at every size. Renames, moves, trashing and restoring inside Files carry it along; deleting for good drops it. Listings, places and search results carry each folder's `look`.
  - **More views:** gallery (one item large, arrows on hover), cards, and disk usage (a squarified treemap, `Layout.js`, of the folder with folders sized in the background). Adaptive (on by default, ☰ menu): a folder with no view of its own and none lent gets one to suit it, mostly pictures a gallery, a code project a list (`Layout.adaptive`), and nothing's remembered until you change it. Item size (Ctrl+= / Ctrl+- / Ctrl+0) is one setting for every view and window.
  - **Arranging:** group by kind, date modified, size or name (`Arrange.js`) is part of a folder's view, like sort; headings fold their group. Grouped grid and cards are rows in a ListView (`GroupedGrid.qml`, the cells shared with the plain grid as `GridCell` / `CardCell`) since a GridView can't hold headings. The list's columns (size, modified, created, kind), their widths and where folders go are one setting for every window (`views.json`).
  - **Menus:** the kit `Menu` takes submenus (an item with `items`), opening beside it, to the left when there's no room. A chosen action runs after every menu has closed, since it may change what the menus hold.
  - **The picker** (`src/choose.rs`, `qml/ChooseStrip.qml`): `whiterose-files --choose …` is Files with a strip above the files saying what's wanted. Panes list only folders and files matching the filter shown (globs on the name or MIME types, `Fs.chooseMatches`); opening a file chooses it; the answer is printed (a line each, or `--json`) and the process exits 0, or 1 for Cancel, Esc or closing the window. A picker restores and saves no tabs and leaves the index alone. Its window title ends in " — Files", as Qt adds the app's name to titles that don't.
  - **Settings picks with it:** `shell/settings/FilePicker.qml` runs `whiterose-files --choose --json` (`$WHITEROSE_FILES` if set) for the user picture, wallpapers (an image, a folder), folders left out of the search index and Wi-Fi certificates, in place of Qt's file dialogs.
  - **Every app's Open and Save** (`daemon/internal/portal`, `whiterosed portal`): WhiteRose's xdg-desktop-portal backend, `org.freedesktop.impl.portal.desktop.whiterose`, implements only the FileChooser (OpenFile, SaveFile, SaveFiles); `whiterose-portals.conf` sends that to it and everything else to GTK's. D-Bus starts it when the portal first asks (`/usr/share/dbus-1/services/…whiterose.service`, running `/usr/bin/whiterosed portal`, so the session's version via `WHITEROSE_DIR`). Each request runs `whiterose-files --choose --json` with the app's title, folder, name and filters (the one it wants shown first; GTK's `*.[pP][nN][gG]` globs work), answers `file://` URIs, and its Request's Close kills the picker. The session sets `GTK_USE_PORTAL=1`, `GDK_DEBUG=portals` and `QT_QPA_PLATFORMTHEME=xdgdesktopportal` so GTK and Qt apps ask the portal; Electron apps do anyway. Choices (extra check boxes) aren't shown.
  - **Settings › Files** (`shell/settings/modules/files`, five sub-pages): everything that isn't about one folder is a whiterosed setting (`files.*`, `apps.terminal`, `apps.editor`), so Settings and every Files window follow a change at once. Files keeps only per-folder things (views, looks, the session). "Make again" sets `files.reindex` (a time). whiterosed (`internal/filesindex`) runs `whiterose-files --index` then, two seconds after what's indexed changes, and every 10 minutes checks whether the index is over an hour old, making it only when plugged in (`/sys/class/power_supply`: something other than a battery online, or no battery). So the index stays fresh with Files closed and costs no battery. `files_index` gives Settings the index's age and progress. `$WHITEROSE_FILES` names the Files to run (the UI tests and `make run-*` use the checkout's build); without one on PATH there's no indexing from the daemon. The shortcut list (`FilesKeys.js`) lives in the Settings module and is compiled into Files, so both show the same.
  - **Tags** (`src/tags.rs`): on the file itself as the `user.xdg.tags` extended attribute (comma-separated; KDE's Dolphin reads the same), so they survive any app renaming or moving the file; copies made by Files carry them. Names and colours are the `files.tags` setting. Files keeps a list of what's tagged where (`$XDG_STATE_HOME/whiterose/files/tags.json`) for the Sidebar counts, `tag:Name` pages and `tag:` searches, kept up by Files' own operations, checked against the files when read, and filled in with other apps' tags when the search index is made. Drives that can't hold extended attributes (some FAT/exFAT sticks) refuse tags, and Files says so.
  - **Git** (`src/git.rs`): `git --no-optional-locks status --porcelain=v2 --branch -z --ignored=matching` in the repository the folder is in, on a background thread at idle priority; each entry's state (a folder: the worst inside), the branch and ahead / behind in the status bar. Asked when a folder is listed and every 4 s while Files is in front and the folder is in a repository; off with `files.git`.
  - **Safety:** Files never runs code by itself (`src/run.rs`): programs, executable scripts and launchers ask first; from untrusted places (Downloads, Desktop, removable drives, temp folders, the Trash, someone else's files) running needs "I trust this file", and `.desktop` launchers from there never run. Archives extract in Rust (`src/archive.rs`) into a hidden folder renamed into place: any absolute or `..` name refuses the whole archive, nothing is written through a link, links pointing outside are skipped, setuid / devices are dropped, and it stops before the drive is nearly full. Drives: `lsblk` lists plugged-in filesystems, `udisksctl` mounts (the polkit agent asks if needed) and ejects (unmount, then power-off when nothing on it is mounted). Everything done or refused goes in `files.log` (0600); ☰ › Help › Copy diagnostics puts version, system, tools, settings and the log's end on the clipboard.
  - **Rule for dialogs:** a popup closed from inside a key handler (Enter) crashes Qt; close it with `Qt.callLater`.
  - **Clipboard:** files go as `text/uri-list`, `x-special/gnome-copied-files` and plain paths (`cpp/clip.cpp`), so copy and paste work with other file managers both ways.
  - **Default:** `/etc/xdg/whiterose-mimeapps.list` makes Files open folders inside a WhiteRose session only.
  - **Views** (`src/views.rs`, `$XDG_STATE_HOME/whiterose/files/views.json`): changing the view or sort in a folder remembers it for that folder. A folder can lend its view to everything under it, and a default covers the rest (own → nearest lending folder above → default). The last 1000 folders set are kept; lending folders always are. Showing hidden files is one setting for every window. The file is read fresh on each lookup, so Files windows in other processes agree.

## Windows

- **Minimise:** a minimised window is hidden but kept, and comes back when it's focused (dock, ALT+TAB, the app's launcher entry). It stays hidden across workspace switches.
- **The session's own processes:** the daemon and shell get SIGTERM when the compositor exits, however it exits, so none is left holding the session's sockets.

## Layouts and tiling

Figma frames: `Settings - Windows` (and scrolled to the end). Plan and decisions: `docs/plans/layout-engine.md`.

- **Three layouts:** `floating` (the default: windows stay where they're put), `dwindle` (each new window splits the focused tile; a split goes side by side when its space is wide, stacked when tall, unless set) and `master` (one large tile on a chosen side, the rest stacked).
- **Which applies:** per workspace *and* output, `monitor setting > workspace setting > global`. Monitors are keyed by their stable ID, as in `displays`.
- **Settings (whiterosed):** `windows.layout`, `windows.layout.workspaces` (`{"1": "dwindle"}`), `windows.layout.monitors` (`{"<monitor id>": "master"}`), `windows.gaps.inner` / `.outer` (0–64 px), `windows.gaps.smart` (no gaps with one window), `windows.master.ratio` (0.1–0.9) and `.side`, `windows.dwindle.split` (`auto | horizontal | vertical`). The compositor follows them live (`layout_setting` in `daemon_message`).
- **The core is pure** (`compositor/src/layout/`): generic over the window type like `workspace.rs`, so trees, rectangles, resizing, neighbours and groups are unit-tested without a compositor. The compositor keeps one `Tiling` per (workspace, output) that isn't floating.
- **One reconcile step:** `retile()` brings the visible workspace in line with the settings and the windows on screen. Windows that should be tiled join next to the most recently used tile, those that shouldn't leave, and each tile gets its rectangle. It runs after a window's first commit (when the app has said whether it's a dialog or fixed size), closing, minimising, switching or moving workspaces, maximize/fullscreen ending, bar or output changes (`refit_windows`) and settings changes. A window is only reconfigured when its rectangle moved, so the bar's frequent commits cost nothing.
- **Floating geometry is kept:** a window's floating place and size are remembered when it's tiled and given back when it floats again (the workspace turns floating, it moves to a floating one). Tiled windows are told they're tiled (xdg-toplevel `tiled_*` states), so apps drop their shadows and rounded corners.
- **What floats on a tiled workspace:** dialogs (windows with a parent), fixed-size windows, and windows you take out (SUPER+SHIFT+SPACE, or dropped outside the tiles). They sit above the tiles. Maximized and fullscreen windows keep their place in the tree.
- **A tiled window is cut to its tile:** a tile can get smaller than an app will go (Ghostty's minimum size, deep in a dwindle split), and the app then draws bigger than its tile. `retile()` records each placed tile (`TilingState.placed`), and `clip_of` makes it the window's clip: `render.rs` builds the space's elements itself and crops the window's own surfaces to it (its popups stay whole, so a menu can reach past the tile), `window_under` hit-tests the pointer by it, and frame callbacks go only to outputs the tile is on. Floating, maximized, fullscreen and dragged windows aren't clipped.
- **Mouse:** a title-bar drag or SUPER+left-drag drags a tile, with a preview of where it lands. Dropped on another tile's top band it joins that tile's group, on its middle the two swap, near an edge it splits that tile. Dropped outside every tiled area it floats there; anywhere else it goes back. A border drag, SUPER+right-drag (nearest corner) or dragging the gap between tiles moves the splits. SUPER+drag also moves and resizes floating windows.
- **Tab groups:** windows sharing one tile, one shown at a time. The tree holds each group's shown window; the other tabs are hidden like windows on another workspace, so changing tab is a `replace` in the tree. The strip above the tile is a small island (Figma "Grouping — tab strip options", C): a 6 px black band across the tile (14 px below its top, clear of the bar's idle island; its ends rounded on top) with the tabs hanging from it in a 30 px island on concave ears, each tab its app's icon (16 px, faded on tabs not shown) and title, as wide as they need (up to 220 px, sharing the room when there isn't enough), the shown one on a lighter pill; beside the island it's see-through, and the window starts 8 px below (`TAB_STRIP` = 58). Dropping a window on a tile's top 28 px (`GROUP_DROP`) groups them. `tab_strip::layout` places the tabs for drawing and clicks alike. Icons (`app_icons.rs`) are found as the shell finds them: the app id's desktop entry (by file name, StartupWMClass, or the id's last part) gives Icon=, looked up in GNOME's icon theme, what it inherits, hicolor and pixmaps (the smallest PNG big enough, `@2x` folders too); PNG only, so an SVG-only app shows its title alone. Each is read, scaled and premultiplied once per size and kept. **The strip moves** (220 ms, eased out) when its group changes on screen: `retile` compares the new strips with the last (the same group if they share a window; `animate_strips`). A new one appears: the band grows from the middle and the island drops out from under it. One whose tabs or shown tab changed slides there: the island widens or narrows, tabs still in it slide, one gone fades out in the first half and one new fades in after (so they don't cross), the pill slides to the shown tab. One gone goes the way it came, drawn over its window (which has its whole tile at once), from the labels last drawn for it, since its windows may be gone. Switching workspace moves nothing. Each in-between frame is a `Scene` (pure functions: `still`, `appearing`, `changing`; tested) drawn afresh; frames are asked for only while one moves. It's drawn by the compositor: fontdue and the Geist font compiled in, anti-aliased shapes in premultiplied ARGB, each strip drawn once at the output's real resolution and cached until its titles, shown tab or size change, behind every window and in front of the wallpaper. A tab leaves its group when it's dragged out, closed, minimised or moved to another workspace; a group of one ends, and so does every group on a workspace that stops tiling.
- **Keys** (Settings › Keyboard, "Windows"): SUPER+T next layout (saved for the workspace, or for the monitor if it has its own), SUPER+SHIFT+SPACE float or tile, SUPER+arrows focus, SUPER+SHIFT+arrows move (a floating window snaps instead, see Snapping), SUPER+R resize mode (arrows resize; Escape, Enter or any other key ends it), SUPER+J flip a split, SUPER+M make master, SUPER+G group or ungroup, SUPER+. and SUPER+, next and previous tab. SUPER+CTRL+arrows was avoided: with `--mod ctrl-alt` it'd be the same keys as SUPER+arrows. SUPER+TAB is the window switcher (CTRL+ALT+TAB with `--mod ctrl-alt`).
- **The bar:** the `workspaces` event carries every output's layout (`layouts`, by output name); each screen's bar shows its own after the workspace dots and cycles it on a click (`next_window_layout` with the output's name).
- **Saving from the compositor:** a setting the compositor saves (SUPER+T, the display trial's end) before whiterosed has connected, or while it restarts, waits in `daemon_waiting` (the latest per key) and is sent when it connects.

## Snapping

Figma frames: `Snap Picker - SUPER + Z`, `Snap Picker - Maximize button`, `Settings - Snapping`. Plan and decisions: `docs/plans/snap-layouts.md`.

- **Where:** floating windows on floating workspaces (on a tiled one, a drag already splits, swaps, groups or floats). Fixed-size and fullscreen windows don't snap. `windows.snap` turns it off.
- **A snap is a zone**, `[x, y, w, h]` as fractions of the output's usable area (`compositor/src/layout/snap.rs`, pure and unit-tested). Its rectangle uses the tiling gaps, so a snapped half is the same rectangle as a dwindle tile. Zones are refitted in `retile()` when the bar, the outputs or the gaps change; a window is only reconfigured when its rectangle moved.
- **Each snapped window remembers its own size** from before it snapped (`snapping.rs`). Dragging it away gives that size back under the pointer, at the same place across its width; restoring puts it back where the drag started. Maximize, fullscreen and minimise keep the snap. Moving the window to another workspace, or the workspace starting to tile, ends it. Snapped windows are told they're tiled (`tiled_*` states), as tiles are.
- **Dragging** (title bar or SUPER+drag): the pointer reaching a side edge gives a half, near a corner a quarter, the top edge maximizes, and the top edge's middle third opens the picker. An edge with another monitor beyond it doesn't snap, so windows still cross. The drop preview is the one tiles use.
- **Zones:** holding SHIFT during a drag shows the monitor's zone grid instead of the edges, and the window drops into the zone under the pointer (one zone per drop). `windows.snap.zones` holds a grid per monitor id (`{"<monitor id>": [[x, y, w, h], ...]}`); without one, two thirds and a third. A grid has 1–16 zones, each inside the area, at least a tenth of it each way, none overlapping. Zones needn't cover the area: a drop over a gap just floats. The daemon, the compositor and the Settings page all check this; the compositor ignores a bad grid.
- **A grid per workspace:** `windows.snap.zones.workspaces` (`{"<workspace 1-9>": {"<monitor id>": grid}}`) gives a workspace its own grid on a monitor. Drags and the picker use the active workspace's (`Settings::zones_for`): the workspace's own on that monitor, else the monitor's, else the default.
- **Resizing together:** a border drag, SUPER+right-drag or a drag on the gap between snapped windows moves the line they share. Every snapped window on that line follows, as long as it lines up with the others along it, so the middle of four quarters moves all four. Edges on the screen's border don't move, and no window gets narrower than a tenth.
- **The picker:** the layouts (halves, thirds, quarters, two thirds and a third both ways, the monitor's own grid) as small diagrams. It opens three ways:
  - SUPER+Z, for the focused window. The arrows, Enter and Escape work it, and SUPER+Z again closes it.
  - A drag to the middle of the top edge. Dropping on a cell snaps there; moving away closes it.
  - Resting on Maximise in a WhiteRose app (the kit's `AppWindow`) for half a second. The kit sends `snap_picker` with `under_pointer` (`apps/kit/src/compositor.rs`), and the picker opens as a card just below the pointer. It closes once the pointer is well away. Other apps draw their own buttons, so the compositor can't see a hover on them.
  - The compositor places the picker, finds the cell under the pointer (even while a drag holds the pointer) and shows the hovered cell's drop preview. The shell only draws the `snap_picker` event (`shell/SnapPicker.qml`, its own overlay layer), at exactly the tiles and cells it's given.
- **Keys** (SUPER+SHIFT+arrows on a floating window): left or right snaps to that half. Pressed again towards the side it already reaches, it goes on to the next monitor's opposite half or quarter; towards the other side, it gives the window its own size back. Up maximizes; from a half it goes to the top quarter, and from a bottom quarter to the half. Down goes from a half to the bottom quarter, from a top quarter to the half, and from a bottom quarter or maximized back to the window's own size. Down never minimises.
- **Settings › Snapping:** the switch, and each monitor's zone grid edited in place (`shell/settings/modules/snapping/`). Its header picks:
  - **Zones for:** all workspaces (the monitor's grid) or one workspace. A workspace starts from the monitor's grid and saves its own on the first change; Reset takes it back.
  - **The tool**, which stays picked (icons, named for screen readers and tests, with tooltips): Resize drags the lines between zones; Side by side and Stacked split the zone clicked (hovering shows where); Merge joins two zones clicked one after the other; Draw drags out a zone over empty space (its edges snap to 24ths and to nearby zone edges, and it stops at the zones around it); Delete takes a zone away (never the last).
  - **Edit on screen** opens the same editor at full size over that monitor, drawn by the shell: Settings asks it over QuickShell IPC (`shell/bin/whiterose-shell zones edit <monitor> <workspace> <tool>`, which finds this session's shell as `whiterose-settings` finds Settings). `ZonesOverlay.qml` is a top layer `whiterose-zone-editor` where the desktop dims behind translucent zones; the tools and Done are in that monitor's island (`island/zones.qml`, `ZoneTools.qml`), and its bar rises to the overlay layer meanwhile so the island is drawn over the zones. Escape or Done ends it; it saves as it goes (`Zones.js` `save`, shared with Settings). Figma: `Edit zones on screen`.
  - Every change is saved as it's made. `Zones.js` holds the grid logic (splitting, merging, moving lines, drawing, deleting) and is tested by `node shell/tests/run.mjs`.
- **An app's smallest size wins:** a zone smaller than a window's minimum size (420x300 for WhiteRose's own apps) is asked for, but the app keeps its own size.

## Window rules

Figma frames: `Settings - Window rules` (and scrolled to the end). Plan and decisions: `docs/plans/window-rules.md`.

- **What a rule is:** `windows.rules`, a list in order. Each rule matches by `app` (app id) and/or `title`, both regular expressions that match anywhere unless anchored, and sets any of:
  - Placement: `workspace` (1–9), `monitor` (a monitor id, as in `displays`), `float` (`true` floats it, `false` tiles it if it can be tiled), `size` and `position` (pixels or `"N%"` of the usable area; `position` may be `"centre"`), `maximized`, `fullscreen`.
  - Look: `opacity` (`[focused, unfocused]`, 0.1–1) and `onTop`.
  - `zone`: a snap zone to open in, numbered as on the Snapping page.
- **Every match applies:** the rules are walked in order and each match is folded into one set of actions, a later rule winning for what it sets (`compositor/src/rules.rs`, pure and unit-tested). A rule with no pattern, or one that doesn't compile, is skipped with a warning in the log; the daemon refuses to save one (`checkWindowRules`, which compiles patterns with Go's RE2, nearly the syntax of Rust's `regex`).
- **When:** once, on a window's first commit (`window_committed` → `apply_rules` in `window_rules.rs`). That is the first moment its app id and title are known, and it hasn't drawn yet, so the app's first frame is already where and how big the rules say. Editing the rules doesn't move open windows.
- **Placement:**
  - Its first configure went out when it was created (focusing it), so `apply_rules` ends by sending one with what the rules changed.
  - A size is sent in that configure. A position needs the window's size, so it's applied on the commit that brings its first buffer (`place_by_rule`). Both apply only to windows that float; a tile ignores them.
  - Opening on another workspace is quiet: that workspace doesn't come into view, focus stays where it was, and the window isn't told it's focused.
  - A fullscreen window on the output a new window opens on leaves fullscreen so the new window is seen, unless a rule sent the new one to another workspace (this moved from `new_toplevel` to the first commit, after the rules).
  - A maximized or fullscreen rule has no size of its own to go back to yet: restored, the app picks its size, at the cascade.
- **A zone** snaps the window as if it were dropped there (`snap_window`): the grid of its monitor and workspace, floating workspaces only, not fixed-size windows. It refits like any snapped window, and dragged out it gets the rule's size back. A zone the grid doesn't have is ignored; a zone wins over a position.
- **Look:** kept on the window for its life (`Look` in its user data). Opacity is the alpha its surfaces and popups are drawn with (`window_alpha`), focused or not by who has the keyboard; Smithay damages an element when its alpha changes, and a translucent one reports no opaque region, so what's behind is still drawn. Window previews and screenshots of a window stay solid. On top: `restack` keeps three bands, tiles, then floating windows, then windows a rule keeps on top, each in its own order; fullscreen windows and the bar's top layers are still above.
- **Settings › Window rules** (`shell/settings/modules/rules/`): each rule is a card whose header names it (the app id, when its pattern is one app exactly) and sums up what it does; it opens to edit in place, a row per action. Move up and down (order matters), delete. Adding offers a blank rule or the open windows, one per app id, which fills in that app id escaped and anchored. Only complete rules are saved: a new rule stays on the page until it has a pattern, and a pattern that doesn't compile shows why under its row. `Rules.js` holds the logic and is tested by `node shell/tests/run.mjs`.

## Scratchpads

Figma frame: `Settings - Scratchpads`. Plan and decisions: `docs/plans/scratchpads.md`.

- **What a scratchpad is:** `windows.scratchpads`, a list of `{id, name, command, keys, size, place, hideOnBlur}`. `size` is pixels or `"N%"` of the usable area (60% × 50% if unset), `place` is `"top"` (centred under the island, the default) or `"centre"`. A pad may have no key (another shortcut took it); it can't be shown until it has one. The daemon checks it (`checkScratchpads`). The parsing, what the key does in each state, where it goes and the slide's maths are pure and unit-tested (`compositor/src/scratchpad.rs`); the rest is `scratchpad_windows.rs`.
- **Its window is the one its key started:** the key runs the command (`spawn`, keeping the pid), and the first window opened by that process or a child of it (the client's pid, walked up through `/proc/<pid>/stat`) is claimed on its first commit, before the window rules. A start that opens nothing within 10 s is given up. Other windows of the same app are ordinary, and an app that's already open isn't taken. Apps that hand new windows to a running copy must be asked for their own: Settings adds `--gtk-single-instance=false` to Ghostty and `--standalone` to Ptyxis.
- **The key:** not running → start it; hidden → show and focus it; shown but not focused → focus it; shown and focused → hide it; shown on another monitor → bring it to the one under the pointer. Several pads can be shown at once. Closing its window turns the pad off; the key starts it again. A pad removed from the setting lets its window go as an ordinary one.
- **Not on any workspace:** a claimed window is taken out of `Workspaces`, so switching workspace leaves it be, and everything built from the focus history leaves it out: ALT+TAB, SUPER+TAB, the shell's window list (dock, screenshot window picking, Settings' open windows) and other apps' taskbars. `focused_window()` is the pad when one has the keyboard, and switching workspace keeps it there. What would put it elsewhere is ignored (move to workspace, float, group, master, split, fullscreen, SUPER+Z); minimizing hides it.
- **Above everything:** shown pads are a fourth band in `restack`, above windows a rule keeps on top, and they're drawn and take the pointer over fullscreen windows too; the bar's top layers and the lock screen are still above. A pad with `hideOnBlur` hides when another window is focused. Window rules give a pad only their look (`apply_look`).
- **The slide:** 150 ms, eased out, from under the top of the usable area (`sliding_pad` in render.rs draws it offset and cut off there). The window is in place at once, so it can be typed into straight away; only its drawing moves. Hiding gives the keyboard back at once and lets the pointer through while it slides up, and a timer unmaps it at the end. A key mid-slide turns it round from where it is. Frames are asked for only while something slides; `Scratchpads::slide_length` zero turns slides off (the 0.5 effects switch can use it).
- **Keys:** pads' keys are added to the shortcut table (`Bindings::add_scratchpads`), so clashes with other shortcuts are seen, and listed on the Keyboard page as shortcuts `scratchpad:<id>` in a "Scratchpads" group; recording one there changes the pad (`Keys.js` `setKeys`).
- **Settings › Scratchpads** (`shell/settings/modules/scratchpads/`): each pad is a card whose header sums up its key, app, place and size; it opens to edit in place (name, command, key, size, place, hide when it loses focus). The key is recorded by clicking it (`KeyRecorder.qml`); a key another shortcut uses waits for Enter and is taken from it. A new pad starts as the terminal (`apps.terminal`, Ptyxis if unset) with SUPER+` if that's free, and is saved once it has a command. `Pads.js` holds the logic and is tested by `node shell/tests/run.mjs`.

## Keyboard and shortcuts

Figma frame: `Settings - Keyboard & Shortcuts`.

- **Every shortcut is an action with a stable id** (`compositor/src/keybind.rs`, `CATALOG`: `close-window`, `workspace`, `volume-up`, ...), a name, a group and default keys. The table is the defaults plus the user's changes, rebuilt the moment a setting changes.
- **Settings:** `shortcuts.keys` holds only changes (id → keys; an empty list turns one off); `shortcuts.custom` holds your own `{id, name, command, keys}`; scratchpads keep theirs in `windows.scratchpads` (see Scratchpads). Keys are strings: `SUPER+SHIFT+S`, `PRTSC`, `XF86AudioRaiseVolume`, `SUPER+1-9` for the nine workspace shortcuts and `SUPER+ARROWS` for the four direction shortcuts. A ranged shortcut is recorded with one of its keys; the settings page knows which by the row's `range` (`digits` or `arrows`).
- **ALT+TAB's apps:** `windows.appSwitcher.workspaces` (Settings › Windows › Switching): `all` (the default) lists apps with windows on any workspace, `current` only those on the active one (`switcher_mru` filters the focus history before grouping by app). SUPER+TAB always lists every window.
- **Monitors by arrow:** SUPER+CTRL+arrow (`move-to-monitor`, `output_move.rs`) takes the focused window to the monitor on that side (`toward`: past that edge, lined up with it most, then nearest; tested). Tiled, it leaves its group and tiles and joins the other monitor's; floating or snapped (unsnapped), it keeps its place relative to the screen, kept inside it (`carried`); maximized, it fills the other screen; fullscreen, it goes fullscreen there. Where it floats before tiling or maximizing moves with it. It stays focused; scratchpads stay.
- **Workspaces by arrow:** SUPER+ALT+arrow (`move-to-workspace-step`) takes the focused window to the workspace before (left, up) or after (right, down) and goes there with it, still focused; nothing at the first or last.
- **Nested, SUPER becomes CTRL+ALT** (when the host keeps SUPER): a shortcut whose remapped keys would be another's (SUPER+ALT+arrow and SUPER+arrow both CTRL+ALT+arrow) keeps its SUPER keys, out of reach in that mode, rather than taking the other's.
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

- Releases are GitHub Releases of the repository in `/usr/share/whiterose/release-repo`: the two packages for each system (`.deb`, `.rpm`), `SHA256SUMS`, and `SHA256SUMS.sig`, an ed25519 signature by the release key (`packaging/release.pub`, installed to `/usr/share/whiterose/release.pub`; `make release-key` makes it once, `make release` signs and publishes). The version, the repository and each release's notes come from `project.yaml` (`make release-notes` shows what will be published, `make changelog` writes CHANGELOG.md from it); a version such as `0.6.0-alpha.12` is published as a pre-release, and packaged as `0.6.0~alpha.12` so dpkg and rpm sort it before `0.6.0`.
- `whiterosed` (`internal/updater`) checks on a schedule (`update.check`: daily by default, weekly, or manually; looked at hourly, never in the first minutes after login). A newer release is downloaded straight away into `~/.cache/whiterose/updates/<version>/`, as you, unless the connection is metered (NetworkManager), when the page offers Download instead.
- Each package downloads under a temporary name; the checksums' signature, then every package against the signed checksums, must pass before anything is kept. A partial, corrupted or tampered download is deleted. The page shows this as its first three steps: Downloaded → Signature (the key's id) → Files verified.

### 2. Install alongside, never over

- The **compositor, daemon, shell and apps** are packaged per version, the same way the kernel is: `whiterose-core-0.5.0` installs into `/usr/lib/whiterose/0.5.0/` (`bin/`, `shell/`, `VERSION`).
- `/usr/lib/whiterose/0.4.2/` is left completely alone.
- A small `whiterose` package holds what doesn't change between versions: `whiterose-session`, the `/usr/bin` commands, desktop entries, icons, PAM / polkit / portal files, the release key. It depends on its version's core.
- **Never more than two versions on disk** (`update.Keep`): the newest, plus the one you're running if that's another (after a rollback), else the one before the newest. Anything older is uninstalled when an update installs. Nothing piles up.
- **Which version runs:** `/usr/lib/whiterose/select` picks `$WHITEROSE_DIR` (set by the session launcher for the whole session), else `current=` in `~/.local/state/whiterose/version`, else the newest installed. The `/usr/bin` commands are three-line wrappers through it, so a session keeps using its version while a newer one installs.
- Installing never changes what runs. Only the session launcher decides which version starts.

### 3. Switch on the next session start, with a health check

- `whiterose-session` (the login screen's session file points at it) sets the session's environment and runs `/usr/lib/whiterose/launch` (`daemon/cmd/whiterose-launch`, logic in `daemon/internal/launch`, tested). It lives outside the versions because it chooses between them. All of it runs as you; nothing needs root.
- Its state is per user, in `~/.local/state/whiterose/version`:
  ```
  current=0.4.2      the version that works (what select picks outside a session)
  pending=0.5.0      a newer one to try
  attempts=1         tries of it so far (at most 2)
  failed=0.5.0       the last one that didn't come up, and why (for the prompt)
  reason=compositor exited during start (signal 11, segmentation fault)
  skip=             a version not to try (you stayed, or rolled back from it)
  snapshot=1762790000
  rollback=on        automatic rollback (off: switch without the health check)
  ```
- A newer version installed (by the Updates page, or apt / dnf) becomes `pending`, unless it's the one that already failed and you chose to stay. A pending or current version that's been removed is forgotten.
- With `pending` set and fewer than two tries, it snapshots the settings and starts that version with `WHITEROSE_DIR`, `PATH` and `WHITEROSE_READY_DIR` set. **Health check:** the compositor (`compositor/src/ready.rs`, before its event loop), `whiterosed serve` (once its socket is up) and the shell (when it connects to the compositor) each write a file in `WHITEROSE_READY_DIR` within 20 s, and the session is still running at 60 s (or ended normally once up: logged out, restarted).
- **Pass:** it becomes `current`; the version before stays as the rollback target until the next update removes it.
- **Fail:** the session is stopped (its process group: SIGTERM, then SIGKILL after 5 s), `failed` / `reason` are recorded, `update-<version>.log` gets the reason and the last 200 lines it printed, the settings snapshot is put back, and `current` starts straight away, in the same login.
- Logging out during the check doesn't count as a try.
- After falling back, the shell shows the recovery prompt: **Try again** (two more tries), **Stay on 0.4.2** (not tried again until a newer one comes), or **View log**. After 2 failed tries it stays on the old version and only offers the choice.
- **Restart session now:** IPC `{"cmd":"restart"}` makes the compositor exit with code 42; the launcher runs the decision again and starts a new session (on the pending version, if there is one). Any other exit ends the login.
- `WHITEROSE_DIR` set when it starts (a build being worked on) runs that directly, no checks.

### 4. Config safety

- Before a try, `~/.config/whiterose/` and the settings database (`~/.local/share/whiterose/settings.db` and its journal) are copied to `~/.local/state/whiterose/snapshot/`. There's one snapshot only; each new try replaces it.
- Config files carry a schema version. Migrations only move forward and run on first start of the new version. A failed try restores the snapshot, so the old version never reads a config format it doesn't understand.

### 5. Installing, and the page

- **Install** (asks for an administrator's password: polkit action `org.whiterose.update.install`) runs `whiterosed update-root DIR KEEP` through pkexec. As root it copies the download into a folder of its own, checks it again (signature by the installed key, every package against the signed checksums; nothing in your folder is trusted), installs both packages with `apt-get` / `dnf` beside the versions already there, then uninstalls anything older than the two kept (`update.Remove`). The download is then deleted.
- **Settings › Updates** (`shell/settings/modules/updates`; Figma `Settings - Updates`): your version and when it last checked, Check now; the release card (what's in it, size, date; the five steps; Download / Install / Apply at next login / Restart session now); Previous version with Roll back (the previous one from the next start, the newer one skipped until a newer release; Stay undoes it); Automatic rollback (off: a new version is switched to without the health check); Settings snapshot (when); Check for updates. The sidebar shows a 1 while there's an update or a failed start to see to. Restart session now (after a confirmation) is offered only when the launcher started the session (`WHITEROSE_LAUNCHER`), as it's what starts the next one: the daemon sends the compositor `{"cmd":"restart"}`.
- **"Couldn't start"** (`shell/RecoveryView.qml`, an island view; Figma `Recovery - Update Failed`): shown once per session while a failed start is recorded. Try again (two more tries, restarting the session now), Stay on the working version (the failed one is skipped until a newer release), View log. It's a question, so it doesn't close when you click elsewhere. Settings › Updates shows the same choice.
- For trying it from a build: `WHITEROSE_VERSION`, `WHITEROSE_ROOT`, `WHITEROSE_RELEASE_KEY`, `WHITEROSE_RELEASE_REPO` and `WHITEROSE_UPDATE_API` override what the daemon would read from the installed system.

### 6. Reboots

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
- `switcher`, which the compositor drives while ALT+TAB / SUPER+TAB is held
- `note` (SUPER+SHIFT+N; the Note module)

SUPER+V opens `clipboard` directly.

- **Features as modules:** an island feature that isn't core is a Settings module like an optional page (`module.json` with `"core": false`, on by default): its page holds its options, and turning it off in Settings › Modules (`settings.modules.disabled`) turns the feature off too (its view doesn't open, its key does nothing). The note is the first; the agenda will be the next.
- **The note** (`shell/NoteView.qml`, logic in `shell/Note.js`, tested): one text, `note.text` in whiterosed, saved 0.4 s after you stop typing and when the island closes. Its first line is the title; `[ ]` / `[x]` lines are tasks. With tasks it opens on the checklist (Figma "Island v2 — Note · tasks": the title, done/total, a segment a task, a tick each; clicking one ticks it in the text); typing or Enter writes, Esc goes back to the checklist or closes. Without tasks it opens writing. Settings › Note clears the finished tasks or the whole note.

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
- **Fingerprint:** a second PAM conversation (`shell/pam/whiterose-fingerprint`, pam_fprintd) runs alongside the password one, so either unlocks (`shell/Auth.qml`). With no reader or no enrolled finger it fails at once and isn't shown. It stops after 30 s or three misses, as the reader draws power while listening; a key or click starts it again.
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
- **Shared components** live in `shell/settings/`: cards, rows, toggles, dropdowns, segmented control (text, or icons with tooltips), fields, sliders and dialogs.
- **As a tile:** Settings works from a third of the screen up (Figma: `Settings - narrow · …`). Below 900px the sidebar becomes an icon rail (`Settings/Rail`); its Menu or Search button, or `/`, slides the full sidebar over the page, and picking a page, the dimmed page or Escape slides it away. Rows (`SettingRow`) and page headers (`PageHeader`) put their controls under their title where they don't fit beside it, and pages with their own grids reflow. `make shots` shows every page at a third, a half and two thirds of the screen.
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
| Mouse & Touchpad | `mouse.*` and `touchpad.*` settings (scroll direction, speed −1 to 1, acceleration, tap to click), applied by the compositor (`pointer.rs`) through libinput to every pointer device as it appears and again on a change; a device that can tap is a touchpad, any other pointer a mouse. Real hardware only: nested, the host desktop's settings apply. `mouse.shakeToFind`: shaking the pointer (four turns of at least 60 px within 800 ms, no button held) grows it to 3× for a moment (`shake.rs`, pure and unit-tested; `cursor.rs` draws the theme's big image at the size wanted, an app's own pointer as the arrow). Frames come on a one-refresh timer only while it's bigger than usual, so a still desktop still draws nothing. `mouse.fasterWith` (Ctrl+Shift to start with; Ctrl+Alt, Super+Shift or off) and `mouse.fasterBy` (2–4×): while exactly those modifiers are held, mouse and touchpad movement is multiplied (`on_pointer_relative`); it's checked on every movement, so letting go is back to normal at once. Nothing is taken from apps, so their Ctrl+Shift shortcuts still work. |
| Appearance | `appearance.*` and `wallpaper.*`; the wallpaper list from `find` in the folders. |
| Privacy | `privacy.*` settings (compositor idle, daemon sleep lock); clears via the compositor and settings. |
| Users | `whiterosed` users ops (AccountsService); permission through the shell's polkit prompt. |
| Top Bar | The `bar.*` settings. |
| Windows | The `windows.layout*`, `windows.gaps.*`, `windows.master.*` and `windows.dwindle.split` settings; monitors from the compositor's `outputs` event. |
| Snapping | `windows.snap`, `windows.snap.zones` and `windows.snap.zones.workspaces` (each monitor's zone grid, and workspaces' own, edited in place or on screen); monitors from the `outputs` event. |
| Window rules | `windows.rules`; open windows from the compositor's `windows` event, monitors from `outputs`. |
| Scratchpads | `windows.scratchpads` (keys included; a clash may change `shortcuts.*` too); the terminal from `apps.terminal`. |
| Displays | Compositor `outputs` event; Apply writes the `displays.test` trial, Keep writes `displays`. |
| Modules, About | The module list and memory use; hardware and OS from `/proc`, `/sys` and `/etc/os-release`. |

- **Pages not built yet** don't appear in the sidebar: Updates, which waits on the update system.
