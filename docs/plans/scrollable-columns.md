# Plan: scrollable columns (v0.3)

**Status: dropped** (2026-11-12). The scrolling layout was built (commits
1-5 here) and then removed: the user doesn't expect to use it, so it left
the roadmap. What stays from this branch: tiled windows cut to their tiles
(an app that won't shrink to its tile no longer covers its neighbours), the
Snapping tools, `make shots`, and the bar's solid glyphs; their decisions
are recorded below, with the scrolling design kept as a record.

Branch: `v0.3/scrollable-columns` (off `v0.3-release`). Roadmap item:
"Scrollable columns" (0.3 · Layouts). Window rules and scratchpads come
later, on their own branches.

## What the roadmap asks for

- niri's model: each workspace is a horizontal strip of columns. New
  windows open to the right of the focused one.
- Column widths (1/3, 1/2, 2/3, full), stacking windows in a column, and
  centring the focused column.
- Scrolling with keys, a touchpad or the mouse wheel.

## Answers (2026-11-12)

- **Scrollable columns come next**, before window rules and scratchpads.
- **Centring is a setting**, `windows.scroll.center`:
  - `never`: scroll just enough to show the column.
  - `overflow` (the default): the same, but centre the column when it
    can't fit on screen next to the column focused before it (niri's
    `on-overflow`).
  - `always`: the focused column is always in the middle.
- **Widths:** a new column is 1/2 of the usable width. The width key cycles
  1/3 → 1/2 → 2/3. Full width has its own key. These are fixed values, not
  settings.
- **Keys:**
  - SUPER+W cycles the column's width.
  - SUPER+SHIFT+W makes it full width (press again to go back).
  - SUPER+[ pulls the window from the next column into this one, under
    the focused window.
  - SUPER+] pushes the focused window out into its own new column, to the
    right.
  - SUPER+arrows (focus) and SUPER+SHIFT+arrows (move) work on columns:
    left and right go between columns, up and down within one.
  - In resize mode (SUPER+R), left and right change the column's width,
    and up and down change the window's height within its column.
- **Scrolling:** SUPER with the mouse wheel or a two-finger touchpad scroll
  moves the strip. A touchpad scroll moves 1:1 with your fingers. Without
  SUPER, the scroll goes to the app as usual. Three-finger swipes come with
  0.4's gestures.

## How things are today (read before starting)

- `compositor/src/layout/mod.rs`: `Layout` (floating, dwindle, master;
  `ALL` is SUPER+T's cycle), `Tiling<W>` (one enum arm per layout:
  insert, insert_at, remove, swap, replace, arrange, resize_edge, ...),
  `neighbour`, `Gaps`, and `Settings` (the `windows.*` keys).
- `compositor/src/layout/dwindle.rs` and `master.rs`: the existing
  layouts. They're pure and unit tested, as the new one will be.
- `compositor/src/tiling.rs`: `retile()` keeps one `TileSet` per
  (workspace, output) and calls `place()` on each window. It already
  remembers floating geometry, handles groups, and tracks `dragging` and
  `preview`. `focused_tile()` and `change_tiles()` are the helpers the key
  actions use.
- `compositor/src/render.rs`: `output_elements` takes the whole `Space`
  for each output. A window mapped past an output's edge would show on the
  monitor next to it. Scrolling makes that a normal case, so it needs
  handling (see Design).
- `compositor/src/input.rs`: pointer axis events (wheel, touchpad
  scrolling) go straight to the client.
- `compositor/src/keybind.rs`: the `Def` table. SUPER+W, SUPER+SHIFT+W,
  SUPER+[ and SUPER+] are free. `parse_key` takes xkb names
  (`bracketleft`), so the defaults are written as `SUPER+BRACKETLEFT` or
  we add `[` and `]` aliases.
- `daemon/internal/store/keys.go`: `layouts` (the allowed layout names)
  and the `windows.*` keys.
- `shell/Bar.qml` (the layout button's icon), `shell/Icon.qml`, and
  `shell/settings/modules/windows/Page.qml` (the layout choice with its
  little diagrams, and the dwindle and master rows).

## Design

**The layout:** `Layout::Scrolling` (`"scrolling"` in settings and IPC)
joins `ALL` after master, so SUPER+T, the bar button and per-workspace or
per-monitor overrides all work as they do for the others.

**The strip (pure, `layout/scroll.rs`, `Scroll<W>`):**
- A list of columns. Each column has a list of windows, a width (a
  fraction of the usable width: 1/3, 1/2, 2/3, or full), and a height
  share per window.
- The view offset: how far the strip is scrolled, in pixels from the
  first column's left edge.
- `arrange(area, gaps)` gives each window's rectangle, and some of them
  are outside `area`. Inner gaps go between columns and between windows
  in a column. Outer gaps go around the visible area, not around the
  whole strip.
- `ensure_visible(w, area, mode)` sets the offset for the focus and the
  centring setting. `scroll_by(px)` clamps it, so the strip never scrolls
  past either end.
- Widths are fractions, so a monitor or bar change rescales the columns.
  The offset is then recomputed so the focused column stays in view.
- `resize_edge`: a left or right edge changes the column's width (freely
  with the mouse, so not limited to the presets). A top or bottom edge
  moves the split between two windows in a column.
- A tab group is one window in a column, like a tile in dwindle.

**Off-screen windows:** (changed in commit 2; see "Along the way")
- A strip window stays mapped in the `Space`, past its output's edge if
  it's scrolled off. Its clip is its own output.
  - `render.rs` draws it on that output only.
  - `window_under` (all pointer hit-testing) ignores it outside the clip.
  - It gets frame callbacks (and its scale) only from that output, and
    only while some of it shows. So a window scrolled out of sight stops
    drawing and costs nothing, unless a live preview is exporting it.
- Its output is its tile set's (`output_of` asks the `TileSet` first), not
  whatever the off-screen position would suggest.
- The window switcher, the dock and the foreign-toplevel list still show
  every window. Activating one focuses it, and the strip scrolls to it.

**Focus follows into view:** whenever focus moves to a window in a strip
(a click, a key, ALT+TAB, a new window opening), the strip scrolls to show
it. A new window opens in a new column to the right of the focused one,
1/2 wide.

**Mouse:**
- Dragging a tile works as in the other layouts:
  - Dropping on the left or right side of a tile makes a new column there.
  - Dropping on the top or bottom half puts it in that tile's column.
  - Dropping in the middle groups the two (tabs).
  - Holding a dragged tile near the left or right edge of the output
    scrolls the strip.
- Dragging the gap between columns changes the width of the column to its
  left. Dragging the gap between windows in a column moves that split.
- SUPER with a wheel or touchpad scroll moves the strip. A wheel click
  scrolls by one column; a touchpad scroll moves 1:1.

**No animations in this branch.** Scrolling jumps straight to its new
position, as tiling and snapping do. Animations come with 0.4's overview.

**Settings › Windows:**
- "Scrolling" joins the layout choice, with its own diagram.
- A "Centre the focused column" row: Never, When needed, Always.
- The bar's layout button gets a `layoutScrolling` icon.

## Commits (stop after each; the user commits)

1. **Strip core (pure):**
   - `layout/scroll.rs`: columns, widths, offset, `arrange`,
     `ensure_visible` for each centring mode, `scroll_by`,
     insert/insert_at/remove/swap/replace, `resize_edge`, cycle width,
     full width, and pulling a window into or pushing it out of a column.
   - `Layout::Scrolling` and `Tiling::Scroll`.
   - `windows.scroll.center` in `Settings`.
   - Unit tests only, with no behaviour change.
2. **Strips in the compositor:**
   - `retile()` arranges strips.
   - Off-screen windows are unmapped, and partly visible ones are clipped
     (render crop and hit-testing).
   - `output_of` asks the tile set first.
   - Focus scrolls the strip, and new windows open to the right of the
     focused column.
   - `"scrolling"` and `windows.scroll.center` in
     `daemon/internal/store/keys.go`, read through `daemon_message`.
   - Integration tests in `compositor/tests/scrolling.rs`.
3. **Keys:**
   - SUPER+W, SUPER+SHIFT+W, SUPER+[ and SUPER+].
   - SUPER+arrows and SUPER+SHIFT+arrows on columns.
   - Resize mode on columns.
   - The Keyboard & Shortcuts page lists the new keys (from the `Def`
     table).
   - Tests.
4. **Mouse and touchpad:**
   - SUPER+wheel and SUPER+two-finger scrolling.
   - Drops into columns.
   - Gap drags.
   - Scrolling while a dragged tile is held near the edge.
   - Tests, using the debug pointer and axis events.
5. **Shell:**
   - Figma frames first: Settings › Windows with Scrolling, and the bar
     icon.
   - The layout choice and the centring row in Settings › Windows.
   - The bar icon.
   - UI tests: pick Scrolling for a workspace, open three windows, check
     that the third is in view and the first is not, then SUPER+W and
     check the width.
6. **Docs:** ARCHITECTURE.md (a "Scrollable columns" part under "Layouts
   and tiling"), the ROADMAP tick, and this plan marked done.

## Not in this branch

- Animations (0.4).
- Three-finger swipes and the zoomed-out view of the strip (0.4: touchpad
  gestures, overview).
- Moving whole columns between workspaces in one go. A window moves on its
  own, as it does today.
- Per-app default column widths (they come with window rules).

## Along the way

- (commit 1) `Layout::Scrolling` exists but isn't in `Layout::ALL` yet, so
  `parse`, SUPER+T and the settings can't pick it until commit 2 (when the
  daemon accepts `"scrolling"` and the compositor can show a strip). In
  `centre: always`, the strip can scroll so the first or last column sits
  in the middle, with empty space beside it; otherwise it stays against
  the left edge and stops at the right end.
- (commit 2) Off-screen strip windows are **not unmapped**: everywhere in
  the compositor, "mapped" means "on the visible workspace" (`retile`'s
  members, the switcher, idle, snapping), so unmapping would drop them
  from the strip. They stay mapped and are clipped to their output
  instead. That needs no crop element: the clip is the whole output, and
  the framebuffer already cuts off anything past its edge. So
  `render.rs` now builds the space's elements itself
  (`space_elements`, the same order as Smithay's
  `space_render_elements`) to leave a strip window off other outputs.
  The `Space` variant of `OutputElement` is gone; windows are `Surface`
  elements.
- (commit 4) Gap drags in a strip always move a column's right edge: the
  gap between two columns resizes the one on its left, from either half of
  the gap. A wheel click lines the view up with the next column; partway
  through a column, a click back lines up with that column. Edge scrolling
  while dragging a tile runs on a 16 ms timer only while the tile is held
  in the 32 px band at a strip's edge. It keeps the pointer position
  itself: a grab's motion runs with the pointer locked, so reading
  `current_location()` there deadlocks.

## More answers (2026-11-12, after commit 5)

Settings and zone-editor work added to this branch, before the docs commit:

1. **Snapping tools** (done): each monitor's header picks the editor's tool
   (Resize · Side by side · Stacked · Merge), which stays picked; hovering
   shows where a split goes; Merge is two clicks.
2. **Settings at small sizes:** first real screenshots of every page at
   tile sizes (half, third, two thirds, short), then fixes, with Figma
   narrow frames first. Includes the Quick Settings panel, which is taller
   than the bar's layer on the UI tests' screen
   (`TestQuickSettingsOpensSettings` fails on the committed tree).
3. **Draw and Delete zones:** Delete leaves empty space; Draw drags a new
   zone over empty space (edges snap to 24ths and to other zones; it stops
   at neighbours; red when under a tenth or past 16 zones). Zones never had
   to cover the screen; a SHIFT-drop over a gap just floats. The tools
   become icons (named, with tooltips), with labels where there's room.
   **Per-workspace grids**, an opt-in override: (workspace, monitor) grid >
   monitor grid > default, in `windows.snap.zones.workspaces`
   (`{"2": {"<monitor id>": [[x, y, w, h], ...]}}`, same checks); each
   monitor's header gets a "For" choice (All workspaces, Workspace 1-9).
4. **Edit on screen:** a button per monitor opens the zone editor full size
   over that monitor (its own overlay layer), same tools plus Done, Escape
   closes; it edits the workspace you're on. Settings keeps its editor.

## Screenshots and the bar's icons (2026-11-12)

- `make shots` (daemon/uitest/shots_test.go, tag `shots`) writes
  build/shots: the bar's ends with each layout, Quick Settings, and every
  Settings page at a third, a half, two thirds, and half height (Settings
  as a scrolling column), with a contact sheet per size. Scale 1 only: a
  scale seeded in `displays` doesn't reach the headless output.
- What they showed:
  - **Settings below two thirds is unusable.** The fixed 250px sidebar
    leaves a third-wide window about 100px for the page, so every page's
    header, cards and controls are cut off. It needs a narrow layout for
    Settings as a whole, not a fix per page.
  - **Quick Settings:** the screenshot toast stacks into the panel, which
    then runs to about 410px while the bar's layer stays 264px, so its
    lower part can't be clicked (`TestQuickSettingsOpensSettings`).
  - **The bar's icons are soft:** 24px Lucide paths with 2px strokes
    shrunk to 13-16px (strokes about 1.1px, between pixels), and a 13px
    icon centred in the 26px bar sits on a half pixel. Dwindle is a
    smudge; scrolling looks like a film strip.
- **Decided:** the bar gets its own icon set: **solid glyphs on a 16px
  grid** (like macOS's menu bar and GNOME's symbolic icons), drawn 1:1 and
  placed on whole pixels. Lucide outlines stay for Settings and panels.
  Figma first.
- (bar icons) `shell/Glyph.qml` holds the bar's solid glyphs (Figma:
  Components › "Bar Icons — 16px", Bar/*; the Top Bar component uses
  them). Do Not Disturb shows the moon: a solid bell can't show a slash
  through it in one colour. `Trigger` now centres its row on whole pixels.

## Settings at narrow widths (2026-11-12)

- Figma: `Settings/Rail` (Components) and the frames "Settings - narrow ·
  half (Windows)", "· third (Snapping)" and "· menu open".
- Below 900px the sidebar becomes a 64px icon rail (each page a named tab;
  Menu and Search slide the full sidebar over the page, over a dim scrim;
  picking a page, the scrim or Escape closes it; "/" opens it searching).
  The page's side padding drops to 16px under 560px.
- `SettingRow` and `PageHeader` stack by themselves where their controls
  don't fit beside the title (controls under the title; header buttons under
  the description, which wraps). Pages with their own grids reflow: Windows
  (cards 3/2/1, workspaces 3/2/1 columns), Power (chart under the charge,
  modes one per row), Modules (figures one per row, descriptions cut short
  before the switch), Users (buttons under the name), Displays (ID under the
  name), Keyboard (filters wrap under the search box), Top Bar (preview
  leaves out the music), Apps, Bluetooth, Appearance (Wallpaper is a
  SettingRow now).
- `TestQuickSettingsOpensSettings` never failed in the shell: the harness's
  ScrollTo read the bar layer's size once, before the panel had grown it.
