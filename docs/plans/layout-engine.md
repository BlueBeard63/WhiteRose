# Plan: layout engine and tiling (v0.3)

Branch: `v0.3/layout-engine` (off `v0.3-release`). Roadmap items: "Layout
engine" and "Dynamic tiling" (0.3 · Layouts). Snap layouts, scrollable
columns, window rules and scratchpads come later, on their own branches.

## What the user asked for

- Tiling like Hyprland's, with its variants: **dwindle** and **master**.
- Chosen **per workspace**: some workspaces stay floating, others tile. A
  global default, overridden per workspace and per monitor (monitor >
  workspace > global).
- Tiled windows can still be **dragged and resized with the mouse**: drag a
  tile onto another to move or swap it; drag the edge between two tiles to
  resize the split.
- Settings for all of it, a key to cycle the layout, and the bar shows the
  current one.

## How things are today (read before starting)

- `compositor/src/workspace.rs`: `Workspaces<W>`, 9 **global** workspaces
  (one active for every output), stacking order and MRU. Generic and unit
  tested; keep the tiling logic the same way (pure, generic, tested).
- `compositor/src/state.rs`: `WhiteRose`, `switch_workspace`, `hide`/`show`
  (`hidden_at` keeps positions), `focus_window`, `forget_window`,
  `run_action`, `usable_area_on(output)`, `output_of(window)`.
- `compositor/src/window_state.rs`: maximize and fullscreen. Keep floating
  geometry in `restore_geometry` and refit on output or bar changes with
  `refit_windows` (only when the rectangles really change, because the bar
  commits often). Tiling should hook into the same refit.
- `compositor/src/handlers/xdg_shell.rs`: `new_toplevel` cascades new windows
  (the comment says "a real placement policy replaces this later");
  `move_request` and `resize_request` start `grabs/move_grab.rs` and
  `grabs/resize_grab.rs`.
- `compositor/src/keybind.rs`: the `Action` enum and the default shortcut
  table (`def("id", "Label", "Category", &["SUPER+…"])`). Shortcuts are
  configurable from Settings › Keyboard (`keyboard.rs`).
- `compositor/src/daemon_client.rs`: settings arrive from whiterosed as
  `settings` and `setting` events; `daemon_message` routes keys to handlers
  (follow `idle_setting` and `keyboard_setting`).
- `compositor/src/ipc.rs`: `Event::Workspaces { active, count, occupied }` is
  what the bar uses; add the layout there (or a sibling event).
- Settings pages are modules: `shell/settings/modules/<name>/`. There is no
  Windows page yet. Earlier pages followed Figma designs, so check whether
  "Settings - Windows" exists in Figma first, and draw it there if not.
- UI tests: `daemon/uitest/*_test.go`, `make ui-test` (see ARCHITECTURE.md).
  New controls need `Accessible` names (Settings controls take their
  SettingRow's title); `TestSettingsEveryControlHasAName` enforces it.

## Design

**Layouts:** `floating`, `dwindle`, `master`. A `Layout` enum in a new
`compositor/src/layout/` module.

**Resolution:** `layout_for(workspace, output)` = the monitor override, else
the workspace override, else the global default. The default is `floating`,
so nothing changes for anyone who doesn't opt in.

**Settings keys (whiterosed store):**
- `windows.layout`: `"floating" | "dwindle" | "master"`
- `windows.layout.workspaces`: `{"1": "dwindle", ...}`
- `windows.layout.monitors`: `{"<monitor id>": "master", ...}`, keyed by the
  EDID-based monitor id (`monitor_id.rs`), as display settings are.
- `windows.gaps.inner`, `windows.gaps.outer` (px), `windows.gaps.smart`
  (no gaps with one window)
- `windows.master.ratio` (0.55), `windows.master.side`
  (`left | right | top | bottom`)
- `windows.dwindle.split` (`auto | horizontal | vertical`)

The key cycle (SUPER+T) changes the **workspace** override and stores it, so
it survives a restart and Settings shows it.

**Tiling state:** per (workspace, output), a tree owned by the compositor:
- Dwindle: a binary tree. Each split has a direction and a ratio; a new window
  splits the focused tile (direction from its aspect, unless set).
- Master: an ordered list (the first n are masters) plus the ratio.
- The tree code is generic over the window type, like `Workspaces<W>`, and
  computes rectangles from (area, gaps), so it's unit tested without a
  compositor.

**Floating geometry is kept:** when a window gets tiled, remember its floating
rectangle; switching the workspace back to floating restores it (the same
idea as `restore_geometry` for maximize). Windows that float on a tiled
workspace sit above the tiles:
- dialogs and windows with a parent
- fixed-size windows (min == max)
- windows the user toggled with SUPER+SHIFT+SPACE

**What tiling has to handle:**
- **Maximize and fullscreen:** a tiled window can still be maximized or
  made fullscreen. It leaves the arrangement until restored and keeps its
  place in the tree.
- **Minimise:** a minimised window leaves the tree, and returns to the
  focused tile's split.
- **Moving to another workspace or output:** the window leaves one tree and
  joins the other.
- **Refits:** they re-arrange the tiles, in `refit_windows`.

**Mouse:**
- Dragging a tiled window (title bar move request, or SUPER+left-drag) starts
  a tile-move grab. A drop preview shows where it will land, and dropping
  inserts it there (or swaps it with the tile under the pointer). Dragging it
  past the edge of the area pulls it out as floating.
- A resize request on a tiled window, SUPER+right-drag, or dragging the gap
  between two tiles changes that split's ratio live, and its neighbours
  follow.
- SUPER+left-drag and SUPER+right-drag also move and resize floating windows
  (Hyprland's habit), which is handy anyway.

**Keys** (defaults, all changeable in Settings › Keyboard; check for clashes
with the existing table first):
- SUPER+T: next layout for this workspace
- SUPER+SHIFT+SPACE: float or tile the focused window
- SUPER+arrows: focus in a direction
- SUPER+SHIFT+arrows: move or swap in a direction
- SUPER+CTRL+arrows: resize the split
- SUPER+J: toggle the split direction (dwindle)
- SUPER+M: make master (master)

**Bar:** `Event::Workspaces` gains `layout` (the active workspace on the
pointer's output). The bar shows a small icon next to the workspace dots,
named "Layout" with the description "Dwindle" etc. Clicking it cycles the
layout, the same as SUPER+T.

**Settings › Windows** (new module, in the Desktop group):
- Default layout (Segmented control: Floating / Dwindle / Master).
- Per workspace: 1–9, each "Default" or a layout.
- Per monitor: each connected monitor, "Default" or a layout.
- Gaps: inner, outer and smart gaps. Master: ratio and side. Dwindle: split.

## Commits (stop after each; the user commits)

1. **Tiling core (pure):** `layout/` with the `Layout` enum, the resolution
   rules, and dwindle and master trees that compute rectangles (gaps, smart
   gaps, ratios, insert, remove, swap, move in a direction, resize a split).
   Unit tests only, with no behaviour change.
2. **The compositor tiles:**
   - State per (workspace, output); new windows join the tree.
   - Close, minimise, move to a workspace, switching workspaces, maximize
     and fullscreen, and refit on bar and output changes.
   - Floating geometry is remembered and restored. Dialogs and fixed-size
     windows float.
   - Settings keys are read through `daemon_message`.
   - Integration tests in `compositor/tests/` (a headless client, as in
     `layer_shell.rs`).
3. **Mouse:**
   - The tile-move grab with a drop preview (drawn by the renderer), and swap
     or insert on drop.
   - The split-resize grab (resize request, SUPER+right-drag, the gap
     between tiles).
   - SUPER+drag for floating windows too.
   - Tests that drive the debug pointer.
4. **Keys:** the actions above in `keybind.rs` and the default table, with
   SUPER+SHIFT+SPACE to float. Also shown in Settings › Keyboard's list
   (the "Windows" category).
5. **Bar:** the layout in `Event::Workspaces`, and the bar's layout button.
   UI test: clicking it cycles the layout and the stored setting changes.
6. **Settings › Windows:** the Figma design first, then the module. UI
   tests: choosing a per-workspace layout tiles that workspace's windows,
   and every control has a name.
7. **Docs:**
   - ARCHITECTURE.md (the layout module, keys and settings).
   - ROADMAP.md: tick the engine and dynamic tiling items; groups (tabbed)
     are left for later unless done here.

## Not in this branch

Grouped (tabbed) windows; snap layouts and FancyZones; scrollable columns;
window rules (by app, title regex, etc.); scratchpads; animations for tiles
moving (later, with 0.4 Overview).

## Open questions for the user

- Default keys: SUPER+T for the layout cycle and SUPER+SHIFT+SPACE to float
  are proposals. Are these the keys you want?
- Should a window dragged out of a tiled workspace's area float, or snap back?
  (Proposed: it floats.)
- Grouped (tabbed) windows: in this branch or later? (Proposed: later.)
