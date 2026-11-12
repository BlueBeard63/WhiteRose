# Plan: window rules (v0.3)

**Status: in progress** (2026-11-12).

Branch: `v0.3/window-rules` (off `v0.3-release`). Roadmap item: "Window
rules" (0.3 · Layouts). Scratchpads come next, on their own branch.

## Answers (2026-11-12)

- **Matching:** every rule that matches applies, in list order; when two
  set the same thing, the later one wins (as Hyprland). A rule matches by
  app id and title, both regular expressions. Wayland only, so there's no
  X11 class; "initial title" is the title the window opens with.
- **What a rule can do:**
  - Placement: workspace, monitor, size, position (or centred),
    float / tile, fullscreen, maximized.
  - Look: opacity (focused and unfocused), always on top.
  - Open in a snap zone (snapping's deferred per-app rules).
  - No blur and no animations: those wait for 0.5.
- **Built small first:** placement, then look, then zone.
- **Rules apply once, when a window opens.** Editing a rule doesn't move
  windows that are already open.
- **"New rule from this window"** is only in Settings: adding a rule offers
  the open windows to pick from. No shortcut, switcher or dock entry.
- **Settings edits in place:** no dialogs, per the design rules.
- **A rule that sends a window to another workspace stays quiet:** that
  workspace doesn't come into view, and focus stays where it was.
- **Picking an open window in Settings fills its app id only:** titles
  change, so the title is left for the user to add.

## How things are today (read before starting)

- `compositor/src/handlers/xdg_shell.rs` `new_toplevel`: maps the window
  (cascaded), adds it to the active workspace and focuses it. **The app id
  and title aren't known yet:** a client sets them after `get_toplevel`,
  before its first commit. So rules can't run here.
- `compositor/src/tiling.rs` `window_committed`: the first commit (the
  `Committed` marker) is when the window gets its tile. The app id, title,
  parent and size limits are known by then, and no buffer has been drawn.
  **Rules run here.** `handle_commit` in `xdg_shell.rs` sends the initial
  configure on that same commit, so a rule's size, maximized and
  fullscreen can go in the first configure: the app draws its first frame
  at the right size.
- `tiling.rs`: `float_window` and `tiling.floating` (taken out of the
  tiles), `tileable`, `retile`. `layout_setting` reads the `windows.*`
  keys; `daemon_client.rs` `daemon_message` passes settings in.
- `compositor/src/state.rs`: `move_focused_to` (workspace moves: `leave_group`,
  `end_snap`, `hide`, `retile`); `focus_window` raises with
  `space.raise_element`.
- `compositor/src/window_state.rs`: `maximize`, `fullscreen_to`,
  `restore_geometry`.
- `compositor/src/snapping.rs`: zone grids per monitor and per workspace
  (`windows.snap.zones`, `.workspaces`), and snapping a window to a
  rectangle with its snap state.
- `compositor/src/render.rs` `space_elements`: every window is drawn with
  alpha `1.0`, so opacity is one number to pass through, clipped tiles
  included.
- `compositor/src/monitor_id.rs`: the EDID monitor ids that rules use for
  monitors (as `windows.layout.monitors` does).
- `compositor/src/ipc.rs` `WindowInfo` (id, app id, title, workspace):
  the shell's `Compositor.windows`, which Settings can read to offer open
  windows.
- `shell/settings/modules/windows` and `.../snapping`: the Windows and
  Snapping pages. Rules get their own page next to them.
- Regular expressions: `regex-automata` / `regex-syntax` are already in
  `Cargo.lock` (pulled in by `tracing-subscriber`), so adding `regex` costs
  little.

## Design

**The setting:** `windows.rules`, a list in order. Every field but one
match is optional:

```json
[
  {
    "app": "^org\\.gnome\\.Ptyxis$",
    "title": "^Preferences",
    "workspace": 3,
    "monitor": "DEL-41A6-…",
    "float": true,
    "size": [900, "60%"],
    "position": "centre",
    "maximized": false,
    "fullscreen": false,
    "opacity": [1.0, 0.85],
    "onTop": true,
    "zone": 2
  }
]
```

- `app` and `title` are regular expressions; a rule needs at least one. A
  pattern matches anywhere in the text unless it's anchored. A rule with a
  pattern that doesn't compile is skipped, with a warning in the log, and
  Settings shows the error on that rule.
- `workspace` is 1-based. `monitor` is an EDID id; a monitor that isn't
  connected is ignored.
- `size` and `position` are pixels or `"N%"` of the monitor's usable
  area. `position` is `[x, y]` from that area's top left, or `"centre"`.
  A window that tiles ignores size and position.
- `float`: `true` keeps it out of the tiles, `false` tiles it even if it
  would float (not a dialog or a fixed-size window: those still float).
- `opacity`: focused and unfocused, 0.1–1.0.
- `zone`: 1-based zone of the snap grid on the monitor and workspace it
  opens on. Floating workspaces only, as snapping is.

**Resolving (pure, `compositor/src/rules.rs`):** given an app id and a
title, walk the rules in order and fold every match into one resolved
`Rule`, where a later match overwrites a field an earlier one set. Unit
tested without a compositor.

**Applying (first commit):** in `window_committed`, before `retile`:
- **Workspace:** the window moves there without switching to it, and focus
  stays where it was. It opens on the active workspace otherwise, as today.
- **Monitor:** it's placed in that monitor's usable area.
- **Float / tile, size, position, maximized, fullscreen** are set before
  the initial configure goes out.
- The resolved look (opacity, on top) is kept on the window (user data),
  since it lasts for the window's life.
- New windows still focus as today, unless a rule sent them to another
  workspace.

**Look:**
- Opacity goes into `render_elements` as the alpha, focused or unfocused
  by the current focus.
- Always on top: such windows stay above the others on their workspace
  when another window is focused and raised, but stay below the shell's
  top layers and fullscreen windows.

**Settings › Rules** (its own page next to Windows and Snapping, with a
Figma frame first, as for the other pages):
- The rules in order, each a card showing its match and what it does,
  edited in place. The fields are app id, title, then a row per action
  with a toggle to set it.
- Add rule: shows the open windows (`Compositor.windows`) to pick from,
  which fills in the app id (escaped and anchored), or a blank rule.
- Move up / down (order matters) and delete on each rule.
- A pattern that doesn't compile shows its error on the field and isn't
  saved until fixed.

## Commits (stop after each; the user commits)

1. **Rules core (pure):** `rules.rs`, parsing `windows.rules` and
   resolving a window's rule; `regex` added. Unit tests only, no
   behaviour change.
2. **Placement:** rules read through `daemon_message`, applied on the
   first commit (workspace, monitor, float / tile, size, position,
   maximized, fullscreen). A headless test that opens a window with a
   matching app id and checks where it went.
3. **Settings › Rules:** the Figma frame, then the page (list, add from an
   open window, edit in place, reorder, delete). A UI test that adds a
   rule from an open window and checks `windows.rules`.
4. **Look:** opacity and always on top, in the compositor and on the
   page.
5. **Zone:** open in a snap zone, in the compositor and on the page.
6. **Docs:** ARCHITECTURE.md ("Window rules"), the roadmap and this
   plan's status.
