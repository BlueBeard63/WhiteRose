# Plan: bug reports from WhiteRose

**Status: planned** (2026-11-13).

Branch: `v0.3/alpha3-fixes` (off `v0.3-release`), after the Releases tab
and the old-version workflow.

## Answers (2026-11-13)

- **How a report gets to GitHub:** WhiteRose opens the repository's bug
  form in the browser, prefilled (version, build, system summary, what
  happened), and saves a report file with the logs and the snapshot to
  attach. No GitHub token is stored on anyone's machine.
- **Forks:** only verified builds report to BlueBeard63/WhiteRose. A build
  is verified when its files match a release signed by the release key.
  Anything else is told it's a modified build and to report to whoever
  made it (or, if the build names its own repository and key, that
  repository). The old-version workflow also closes reports whose build
  doesn't match a release.
- **What's captured, and when:**
  - a snapshot on every crash (compositor, shell or daemon), kept locally
    (the last 5), nothing sent until you report;
  - a shortcut that takes the same snapshot the moment a non-crash bug
    happens;
  - after a crash, the island offers "Report this", like the update
    failure prompt.
- **Where:** ~~a Settings › About sub-page~~ changed the same day: the
  report is **a step-by-step flow that grows out of the island**, one
  question per step, everything filled in that can be, each answer
  changeable. Settings › About has a "Report a problem" row that opens it.

## Decided while planning

### The session log

Today the launcher keeps only the last 200 lines of the session's output,
in memory, for a failed update. It now also writes the whole session's
output (compositor, daemon, shell: they share its stderr) to
`~/.local/state/whiterose/session.log`, the one before kept as
`session.1.log`. Capped at 8 MB each (the oldest half dropped when it
fills). Reports take the end of it.

### Crash records

- **Compositor:** the session ends. The launcher sees an exit that isn't 0
  (logged out) or 42 (restart), and writes a crash record.
- **Shell or daemon:** the compositor's supervisor already restarts them.
  It now also writes a crash record (which one, exit code or signal, when)
  before restarting.
- A record is a folder in `~/.local/state/whiterose/crashes/<time>-<what>/`
  with `crash.json` and, once whiterosed has taken it, `snapshot.json`.
  The last 5 are kept.
- whiterosed takes the snapshot: straight away for a shell crash; when it
  starts again for its own crash; at the next login for a compositor crash
  (system facts then, plus the log tail from that session, which survives
  in `session.1.log`; the load at the moment is lost, and the report says
  so).

### The snapshot

What's in it (each a section you can untick before reporting):

- **WhiteRose:** version, build (verified / modified / development), how
  long the session had run, Xwayland on or off.
- **System:** distribution and version, kernel, CPU model and threads,
  memory, GPU(s) and driver (from `/sys/class/drm`), monitors (size,
  scale, refresh).
- **Load at the time:** load average, memory and swap used, the 10
  processes using the most CPU and the 10 using the most memory (name and
  numbers only: no command lines, which can hold paths and tokens).
- **Windows:** how many, per workspace, and their app IDs. No titles: they
  can hold document names, chats, pages.
- **Logs:** the last 400 lines of the session log; the update log if a
  version failed to start recently.

Nothing in it is sent anywhere by WhiteRose; the report page shows it
before you open the report.

### Verified builds

- `make release` works out a **build ID** for each package's version
  folder: sha256 of a sorted list of `path  sha256` for every file in
  `/usr/lib/whiterose/<version>`. The ID goes in the signed `SHA256SUMS`
  as `<id>  build-<version>`. (Verifying downloads only checks the
  packages present, so the extra line changes nothing there.)
- whiterosed works out the same ID for the version it runs from (once,
  cached). To call the build verified it fetches that release's
  `SHA256SUMS` and `.sig` from the repository in
  `/usr/share/whiterose/release-repo`, checks the signature with
  `/usr/share/whiterose/release.pub`, and looks for the ID.
- Verified: the report goes to that repository. A fork that ships its
  own repository and key and signs its releases gets reports in its own
  repository, which is what we want.
- Not verified (rebuilt, patched, or a fork that kept this repository's
  name and key): "This is a modified build of WhiteRose. Report it to
  whoever made it." The report file is still saved; no link to this
  repository.
- Releases from before this one have no build line, so they can't be
  verified. Reports work from the release this ships in.

### The bug form and the workflow

- The bug form gets two more fields, both filled in by WhiteRose:
  **Build** (the ID) and **System** (the snapshot's short summary).
  Build is required, and the form says to report from WhiteRose itself
  (SUPER+SHIFT+B, or Settings › About › Report a problem).
- The old-version workflow also checks the build: it reads the release's
  `SHA256SUMS` for the reported version and closes the report with a
  short note if the ID isn't there ("modified build"). Ideas are never
  touched.

### The report: steps in the island

Opened from the crash prompt, the SUPER+SHIFT+B toast, or Settings ›
About › Report a problem. The island grows into a panel (a view like
`recovery`, with the keyboard, as wide as the launcher) and walks through
five steps, one at a time, with Back and Next and a step count ("2 of 5").
Every step starts filled in from the snapshot; you only change what's
wrong. Esc or Not now closes it, and it reopens where you left off until
the report is opened or thrown away.

1. **Which problem.** Picked for you: the crash it was opened from, else
   the newest snapshot from the last 10 minutes, else "Something that's
   happening now" (a snapshot is taken then). The others are listed to
   pick instead ("Shell crashed · 14:02", "Snapshot · 13:40").
2. **What kind.** Crashed / Froze or stopped responding / Looks wrong /
   Doesn't do what it should / Slow. A segmented choice, preset from the
   snapshot (a crash record → Crashed; under memory pressure → Slow).
3. **Where.** A dropdown preset from the snapshot: what crashed, else the
   app that was focused, else the shell piece (bar, dock, island,
   Settings page, Files, Notepad, windows and tiling, displays, another
   app). The title is made from 2 and 3 ("Shell crashed (signal 11)",
   "Files froze") and can be edited here.
4. **What happened.** One text box: what you were doing and what you
   expected. The only step with nothing to fill in for you, and the only
   required one. A line under it: "Steps to make it happen again help
   most."
5. **What's sent.** The snapshot's sections as rows with switches, all
   on, each opening to its exact text; the build line ("Verified build
   0.3.1" / "Modified build: this goes to a file only"). **Open report**
   saves `whiterose-report-<time>.txt` in Downloads, opens the prefilled
   bug form, shows the file in Files to drag into the issue, and the
   island shrinks back with "Report opened in your browser".

The flow's state lives in whiterosed (`report_draft`), so the shell
restarting (or a crash) doesn't lose what was typed.

Designed in a local preview first, like the Releases tab, one step at a
time.

### The shortcut and the crash prompt

- **SUPER+SHIFT+B** takes a snapshot (check it's free in the keybind
  table; it shows under Settings › Keyboard like the others). A toast:
  "Snapshot saved" with Report (opens the flow at step 2) and Later.
- **After a crash** the island shows "Report this" (a view like
  `recovery`), once per crash: "The shell crashed and restarted" with
  Report and Not now. Report opens the page with that snapshot picked.

## Steps (one commit each)

1. Session log in the launcher (`daemon/internal/launch`), capped and
   kept one back.
2. Crash records: the launcher (compositor) and the supervisor (shell,
   daemon) write them; whiterosed's `snapshot` package takes snapshots
   (ops `report_snapshots`, `report_snapshot_now`).
3. Build IDs: `make release` adds them to the signed sums; whiterosed
   works out its own and verifies it (op `report_build`).
4. The report flow in the island (preview first), the report file and
   the prefilled form; the About row; bug form fields.
5. The workflow checks build IDs.
6. SUPER+SHIFT+B and the island's crash prompt.
