# Plan: snap layouts (v0.3)

**Status: done** (2026-11-12), in seven commits on this branch, plus a
fix and a follow-up; what was built is described in ARCHITECTURE.md,
"Snapping". The answers below record the decisions made along the way.

Branch: `v0.3/snap-layouts` (off `v0.3-release`). Roadmap item: "Snap
layouts" (0.3 · Layouts). Scrollable columns, window rules and scratchpads
come later, on their own branches.

## What the roadmap asks for

- Drag a window to an edge or corner and it snaps to a half or a quarter,
  with a preview first.
- A layout picker (halves, thirds, quarters, 2/3 + 1/3) on the maximize
  button or a key.
- Custom zone grids per monitor (FancyZones), edited in Settings.
- Snapped windows resize together when you drag the edge they share.

## Answers (2026-11-12)

- **Snap layouts come next**, before window rules, scrollable columns and
  scratchpads.
- **How the picker opens:**
  - SUPER+Z opens it for the focused window.
  - Dragging a window to the top middle of a screen shows it as drop
    targets (as on Windows 11).
  - In WhiteRose's own apps (the kit's `AppWindow`), hovering the maximize
    button opens it too.
  - Other apps draw their own maximize buttons (there are no server-side
    decorations), so the compositor can't see a hover on them.
- **Floating workspaces only.** On a tiled workspace, a drag already
  splits, swaps, groups or floats, so snapping there would make one drag
  mean two things.
- **Zones are held, not always shown:** a plain drag snaps to edges and
  corners. Holding SHIFT during the drag shows the monitor's zone grid,
  and the window drops into a zone (FancyZones' default).

## How things are today (read before starting)

- `compositor/src/grabs/move_grab.rs`: the floating move grab. It keeps
  the window below the bar (`keep_below`). Snapping hooks into its
  `motion` and `button`.
- `compositor/src/grabs/tile_grab.rs` and `tiling.rs` (`show_drop_preview`,
  `set_preview`, `tiling.preview`): the drop preview the renderer already
  draws. Reuse it for the snap preview; don't add a second one.
- `compositor/src/window_state.rs`: `restore_geometry` keeps a maximized
  window's floating rectangle. A snapped window keeps its pre-snap size the
  same way, and `refit_windows` refits it when outputs or the bar change.
- `compositor/src/layout/mod.rs`: `Rect`, `neighbour`, `Gaps`. The snap
  geometry goes here as pure code (`layout/snap.rs`), tested without a
  compositor.
- `compositor/src/keybind.rs`: SUPER+Z is free. SUPER+SHIFT+arrows moves a
  floating window a step (`move_direction`).
- `compositor/src/ipc.rs`: `Request` and `Event`, which the shell uses.
  `$WHITEROSE_SOCKET` is set for every process the session starts, so kit
  apps can send requests too.
- `apps/kit/qml/AppWindow.qml`: the title buttons of WhiteRose's own apps
  (minimise, maximize / restore, close).
- The Settings › Windows module (from the layout-engine branch) and its
  Figma frames `Settings - Windows`. Snapping gets a section there.

## Design

**Where it applies:** floating workspaces (the layout resolved for the
workspace and output is `floating`). It's on by default, and off with
`windows.snap`.

**Snap targets** (pure, in `layout/snap.rs`):
- Edges: left and right halves. The top edge maximizes, except its middle
  third, which opens the picker targets. A drag to the bottom edge does
  nothing.
- Corners: quarters.
- The trigger is the **pointer** reaching the edge of the output's usable
  area (within a few px), not the window. Monitors that touch each other
  only snap on outer edges, so a window can still be dragged across the
  boundary.
- Snapped rectangles respect `windows.gaps.inner` / `.outer`, so snapped
  and tiled windows look the same.

**Snap state:** each snapped window remembers:
- its zone (as fractions of the usable area, not pixels),
- its output,
- its rectangle from before it snapped.

It's refitted from the fractions on output or bar changes. Dragging a
snapped window away gives it back its pre-snap size under the pointer
(Windows does the same). Maximize, fullscreen and minimise keep the snap,
and restoring returns to it. The snap ends when the window moves to
another workspace or the workspace starts tiling.

**Resizing together:** when two snapped windows on one output share an edge
(within the gap), dragging that edge with a border drag or SUPER+right-drag
resizes both. That updates both windows' fractions. A third window touching
the same line follows too (for example, quarters).

**Zone grids (FancyZones):**
- `windows.snap.zones`: `{"<monitor id>": [[x, y, w, h], ...]}` as
  fractions of the usable area, keyed by the EDID monitor id.
- With no grid, SHIFT shows the default one (the picker's 2/3 + 1/3).
- While dragging with SHIFT held, every zone is drawn faintly and the one
  under the pointer is highlighted. Dropping fills it.
- One zone per drop (no spanning).

**The picker:**
- Layouts: halves, thirds, quarters, 2/3 + 1/3, 1/3 + 2/3, and the
  monitor's zone grid if it has one.
- Each layout is a small diagram; hovering or choosing one of its cells
  puts the window there.
- Drawn by the shell (QML layer surface, island style, from a Figma frame),
  so it matches the rest of WhiteRose and handles keyboard focus. The
  compositor sends the layouts and the cells' rectangles (`snap_picker`
  event) and keeps the hit-testing, so a drag can drop on a cell while
  the pointer is grabbed.
- Opened by:
  - SUPER+Z: for the focused window. Arrows and Enter choose a cell;
    Escape closes it.
  - Dragging to the top middle of the screen: it shows under the edge;
    dropping on a cell snaps there, and dropping anywhere else closes it.
  - Hovering the maximize button in a kit app (after a short delay): the
    kit sends `snap_picker { under_pointer: true }`, and the compositor
    picks the window under the pointer, so the app doesn't need to know
    its own window id.

**Settings › Windows › Snapping:**
- Snap windows (on/off).
- The zone grid per connected monitor: a small editor in place (no dialog)
  that splits a zone horizontally or vertically, drags a split, and merges
  two zones. Reset goes back to no grid.

## Commits (stop after each; the user commits)

1. **Snap core (pure):** `layout/snap.rs`.
   - Edge and corner hit-testing, giving a target.
   - Zone fractions to rectangles (with gaps), and a pointer position to
     a fraction (for edge drags).
   - The picker's layouts.
   - Shared-edge detection among snapped rectangles, and moving that
     edge.
   - Validating zone grids.
   - The SUPER+SHIFT+arrow steps (which snap comes next).
   - Unit tests only, with no behaviour change.
2. **Edge and corner snapping:**
   - The move grab snaps on floating workspaces and shows the existing
     drop preview.
   - Snap state; restore on drag-out; refit on output and bar changes.
   - SUPER+SHIFT+arrows snap floating windows.
   - Maximize, fullscreen and minimise keep the snap. Moving workspace
     ends it.
   - `windows.snap` read through `daemon_message`.
   - Integration tests (`compositor/tests/snap.rs`, using the debug
     pointer).
3. **Resizing together:** a shared-edge resize grab, for border drags and
   SUPER+right-drag on a snapped window. Tests.
4. **Zone grids:** `windows.snap.zones` (`daemon/internal/store/keys.go`),
   SHIFT while dragging, drawing the zones and the highlighted one. Tests.
5. **The picker:**
   - The Figma frame first.
   - `snap_picker` event and request; the shell's picker panel.
   - SUPER+Z, and the drag to the top middle.
   - The kit's maximize-button hover.
   - Keyboard navigation.
   - UI tests: SUPER+Z, choose the left third, and check where the window
     ended up; every control has a name.
6. **Settings › Windows › Snapping:** the Figma frame first, then the
   section and the zone grid editor. UI tests: split a zone, then drag with
   SHIFT and check the window fills it.
7. **Docs:** ARCHITECTURE.md ("Snapping", under Layouts and tiling) and
   the ROADMAP tick.

## Not in this branch

- Snap assist (offering the other windows to fill the rest of the screen).
- Snapping on tiled workspaces.
- Animations (later, with 0.4).
- Per-app snap rules (they come with window rules).

## More answers (2026-11-12, before commit 1)

- **Keys snap:** SUPER+SHIFT+arrows on a floating window (on a floating
  workspace) snaps the way Win+arrows does, instead of moving it a step:
  - Left or right: snap to that half. Again from that side's half or
    quarter: on to the next monitor's opposite half or quarter, if there
    is one. From the other side: give back the window's own size.
  - Up: maximize; from a half, its top quarter; from a bottom quarter,
    its half.
  - Down: from a half, its bottom quarter; from a top quarter, its half;
    from a bottom quarter or maximized, the window's own size.
  - Down never minimises.
- **One zone per drop:** no spanning zones.
- **The top edge maximizes**, except its middle third, which shows the
  picker targets.

## Along the way

- (commit 5) The picker is its own overlay layer, not an island view: the
  compositor works out its tiles and cells and hit-tests them, so a drag
  can drop on one while it holds the pointer; the shell only draws them.
- (commit 6) The `windows` event wasn't re-sent when a Qt app resized
  itself: the commit handler compared the geometry after the commit was
  applied with itself. It now remembers the size it last saw (a fix commit).
- (after commit 6, the user's suggestion) Snapping got its own Settings
  page beside Windows, instead of a section at the end of it.
- A zone smaller than an app's minimum size (420x300 for the kit's apps) is
  asked for, but the app keeps its own size. Nothing breaks; showing it in
  the editor is left for later.
