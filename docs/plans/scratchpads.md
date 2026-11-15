# Plan: scratchpads (v0.3)

**Status: done** (2026-11-12), in five commits on this branch; what was
built is described in ARCHITECTURE.md, "Scratchpads". The answers and
design below are what it was built from; where it ended up differently is
under "As built".

Branch: `v0.3/scratchpads` (off `v0.3-release`). Roadmap item:
"Scratchpads" (0.3 · Layouts), the last 0.3 part.

## Answers (2026-11-12)

- **Which window is the scratchpad:** the one its key started. The key
  runs the command, and the first window that process (or a child of it)
  opens becomes the scratchpad. Other windows of the same app stay normal
  windows. An app that's already open isn't adopted.
- **Showing and hiding slide from the island** now: a short slide down
  from the top centre and back up, just for scratchpads. It becomes part
  of 0.5's animations (and their battery switch) later.
- **Losing focus is a choice per scratchpad:** by default it stays shown,
  above the other windows, until its key is pressed again. A toggle on the
  scratchpad hides it when it loses focus instead (drop-down terminal
  style).
- **It follows you:** a scratchpad isn't on any workspace. The key shows it
  on the monitor you're working on, over the active workspace, and
  switching workspace leaves it shown.

## Decided while planning

- **The key:** not running → start it (it shows when its window opens);
  hidden → show and focus; shown but not focused → focus it; shown and
  focused → hide. Shown on another monitor → it moves to this one.
- **Several can be shown at once;** each has its own key.
- **Where it goes:** a size (pixels or `"N%"` of the usable area, default
  60% × 50%) and a place: `"top"` (centred under the island, the default)
  or `"centre"`. It always floats, never tiles.
- **Above everything else:** above other windows, fullscreen ones
  included (you asked for it with a key), below the shell's top layers
  and the lock screen.
- **Closing its window** forgets it; the key starts it again.
- **Not in the switchers or the dock:** it has its own key, so ALT+TAB,
  SUPER+ALT+TAB and the dock's running apps leave scratchpad windows out.
- **Window rules** still give its look (opacity), but not where it goes
  or whether it floats: the scratchpad decides that.
- **Not started at login:** a scratchpad starts the first time its key is
  pressed.
- **Single-instance apps** (Ghostty's default when started by the
  desktop) hand the window to the process already running, so it isn't
  the one the key started. The command should ask for a new instance
  (`ghostty --gtk-single-instance=false`); Settings fills that in when the
  command is Ghostty. A start that opens no window within 10 s is given
  up, with a warning in the log.

## As built

- **Switching workspace while a scratchpad has the keyboard** leaves it
  there: the pad comes along on top, so it keeps focus (and one that
  hides on losing focus doesn't hide).
- **Another window closing** doesn't take the keyboard from a shown pad.
- **What would put a pad elsewhere is ignored** while it's focused: move
  to workspace, toggle floating, group, master, split, fullscreen (the
  key and the app's own request) and the SUPER+Z picker. Minimizing it
  hides it.
- **The "monitor you're on"** is the one under the pointer, as for new
  windows.
- **A pad removed from the setting** lets its window go: it becomes an
  ordinary window on the active workspace.
- **Window rules** give a pad only their look (`apply_look`).
- **The slide** (`Slide` in scratchpad.rs, drawn by `sliding_pad` in
  render.rs): 150 ms, eased out (cubic). The window is in its place at
  once (so it can be focused and typed into); only its drawing slides,
  cut off at the top of the usable area. Hiding gives the keyboard back
  at once and lets the pointer through while it slides up; a timer
  unmaps it at the end. `Scratchpads::slide_length` zero turns slides
  off (the headless tests do, except the one about slides).

- **A pad may have no key** (the daemon allows it): another shortcut
  can take its only key, from either page. Its card then says it has
  none. Only a command is needed to save a pad.
- **Keys on both pages:** the Keyboard page lists pads' keys as
  shortcuts `scratchpad:<id>` in a "Scratchpads" group (`Keys.js`
  `setKeys` writes them into `windows.scratchpads`); recording there
  works as for any shortcut, and its menu has "Edit scratchpad…". The
  Scratchpads page records with `KeyRecorder.qml`; a clash waits for
  Enter and takes the key from the other shortcut.
- **New instance flags:** a command for Ghostty gets
  `--gtk-single-instance=false`, one for Ptyxis `--standalone` (both
  hand new windows to a running copy otherwise). A new pad starts as the
  `apps.terminal` setting (Ptyxis if unset) with SUPER+` if free.
- **Not done:** pads don't start at login (the first key press starts
  one); apps that always hand windows to a running copy and have no
  flag against it (GNOME Terminal's server, say) can't be scratchpads;
  a pad's dialogs open on the workspace like any window.
- **Figma:** "Settings - Scratchpads" (117:5999) with its note,
  `Icon/scratchpad` and `Nav/Scratchpads` in the sidebar component.

## How things are today (read before starting)

- `compositor/src/keybind.rs`: `Action`, `Bindings::build(terminal,
  overrides, custom)` (custom shortcuts: `CustomShortcut { id, name,
  command, keys }`), `conflicts()`. The keys are parsed with `expand`.
- `compositor/src/state.rs`: `spawn(command)` (split on whitespace,
  `Command::spawn`, the child dropped: its pid is free to keep);
  `run_action`; `hide` / `show` (unmap, remembering where it was);
  `switch_workspace`; `focus_window`.
- `compositor/src/workspace.rs`: `Workspaces<W>` holds every window on a
  workspace; a scratchpad window must not be in it (so switching never
  hides it), or be in it but skipped. Not in it is cleaner.
- `compositor/src/tiling.rs` `window_committed`: the first commit, where
  `apply_rules` (`window_rules.rs`) runs. **Claiming the scratchpad
  window goes here, before the rules.** The client's pid comes from
  `wl_client` credentials.
- `compositor/src/window_rules.rs`: `window_alpha`, `on_top` (stacking
  above), `apply_rules`.
- `compositor/src/render.rs` `space_elements`: per-window location and
  alpha; the slide is a location offset (and a clip at the top of the
  usable area) applied here while it runs. Redraws are damage-driven
  (`schedule_redraw`), so the slide asks for a frame until it ends.
- `compositor/src/ipc.rs` `WindowInfo`: the shell's `Compositor.windows`
  (switchers, dock, Settings' "pick an open window").
- `daemon/internal/store/keys.go`: settings keys with a `Check`
  (`windows.rules` → `checkWindowRules`).
- `shell/settings/modules/rules/`: the Window rules page (cards edited in
  place, no dialogs), the model for the new page.
  `shell/settings/modules/keyboard/`: shortcuts and their keys (`Keys.js`).

## Design

**The setting:** `windows.scratchpads`, a list:

```json
[
  {
    "id": "pad-1",
    "name": "Terminal",
    "command": "ghostty --gtk-single-instance=false",
    "keys": ["SUPER+GRAVE"],
    "size": ["60%", "50%"],
    "place": "top",
    "hideOnBlur": false
  }
]
```

- `id` is stable (Settings makes it), `name` is what Settings and the
  Keyboard page show. `command` and one key are required.
- The daemon checks it (`checkScratchpads`): unique ids, a command, keys
  that parse, sizes in range, no unknown fields.

**Core (pure, `compositor/src/scratchpad.rs`):**
- Parsing the setting.
- Each pad's state: `Off`, `Starting { pid, since }`, `Hidden(window)`,
  `Shown(window)`, and what the key does from each (the table above), as
  a function tested without a compositor.
- Its rectangle on a monitor's usable area (size, top / centre).
- Whether a client pid is the started pid or a descendant of it
  (`/proc/<pid>/stat` parent chain, given as a function so tests can fake
  it).

**Compositor:**
- `Action::Scratchpad(id)`; the pads' keys go into `Bindings::build` so
  `conflicts()` sees them against every other shortcut.
- Starting keeps the child's pid. On a window's first commit, a pad that's
  `Starting` with a matching pid claims it: it floats, is kept out of
  `Workspaces`, gets its rectangle on the monitor you're on, and shows.
- Shown pads stack above everything else; showing one raises and focuses
  it. Losing focus hides a `hideOnBlur` pad.
- Hiding unmaps it (as workspaces do); focus goes back to the window that
  had it before.
- A closed pad window goes back to `Off`.
- Being out of `Workspaces` keeps it out of everything built from its
  focus history: ALT+TAB, SUPER+ALT+TAB, the shell's window list (dock,
  screenshot window picking, Settings' open windows) and other apps'
  taskbars (foreign toplevel). No `WindowInfo` field is needed.

**The slide:** about 150 ms, eased. Showing moves the window down from
behind the top of the usable area (clipped there, so it comes out from
under the island); hiding is the reverse, and it's unmapped at the end. A
key pressed mid-slide turns it around from where it is. Render-time only:
the window's place in the space doesn't change during the slide.

**Settings › Scratchpads** (its own page next to Window rules; a Figma
frame first, as for the other pages):
- Each pad a card edited in place: name, command, key, size, place, hide
  when it loses focus. Add (with Terminal filled in from the terminal
  setting) and delete. No dialogs.
- A key that clashes with another shortcut shows the clash on the card.
- The Keyboard & shortcuts page lists the pads' keys in a "Scratchpads"
  group, which opens the Scratchpads page to change them.

## Commits (stop after each; the user commits)

1. **Core (pure) + setting:** `scratchpad.rs` (parse, state and key
   table, rectangle, pid ancestry) with unit tests; `windows.scratchpads`
   in the daemon with its check and tests. No behaviour change.
2. **Compositor:** keys, start, claim on the first commit, show / hide /
   focus, follow across workspaces and monitors, above everything, hide
   on focus loss, closing; out of the switchers and the dock. Headless tests (a test client
   started by the key is claimed, shown, hidden; switching workspace
   keeps it).
3. **The slide** from the island, in and out, turning around mid-slide.
4. **Settings › Scratchpads:** the Figma frame, the page, the Keyboard
   page's group. A UI test that adds a pad and checks
   `windows.scratchpads`.
5. **Docs:** ARCHITECTURE.md ("Scratchpads"), the roadmap and this plan's
   status.
