# Plan: running out of memory without losing the session

**Status: planned** (2026-11-13), after [bug reports](bug-reports.md),
whose crash snapshots it uses.

## Where it stands

- Nothing in WhiteRose handles memory running out.
- The compositor, shell, daemon and every app run in one cgroup: the
  login's `session-N.scope`. Ubuntu runs systemd-oomd, which kills whole
  cgroups under pressure, so a leaking app can take the session (and every
  other app) with it.
- The kernel's OOM killer, if it gets there first, picks the biggest
  process; nothing steers it away from the compositor.
- A compositor crash ends every app (Wayland clients can't outlive it).
  The shell and daemon are restarted (5 times a minute at most).

## What we're making

1. **Each app in its own scope.** Everything the compositor, launcher,
   dock and Files start runs as `app-whiterose-<app id>-<n>.scope`
   (systemd's StartTransientUnit on the user bus, falling back to a plain
   spawn without systemd). oomd and the kernel then kill one app, not the
   session, and each app's memory is readable from its cgroup.
2. **A kill order, without root.** A user can't lower a process's
   `oom_score_adj` but can raise it, so: apps +300, protected apps +100,
   apps the watcher suspects +800. The session's own processes stay at 0,
   last in line. Scopes get `ManagedOOMPreference=avoid` for protected
   apps.
3. **Protected apps:** the terminal (`$TERMINAL`, the SUPER+RETURN one) by
   default, and any app you mark "Keep running" (Settings, and the window
   menu).
4. **The watcher (whiterosed):**
   - Reads `/proc/pressure/memory` (a PSI trigger, no polling while
     calm) and, under pressure, each app scope's `memory.current` every few
     seconds.
   - **Leak suspicion:** an app whose memory has grown steadily for 10
     minutes while it wasn't the focused one, or that holds more than half
     of memory.
   - **Tight** (some pressure for 10 s, or less than 10% available): the
     island says which app and how much ("Firefox is using 6.2 GB and still
     growing") with Close and Not now.
   - **Critical** (full pressure for 2 s, or less than 3% available):
     closes the worst unprotected app itself (asks it to close, then kills
     it after 3 s), and the island says which and why.
   - Each forced close writes a crash record with a snapshot, so it can be
     reported like a crash.
5. **Settings › Apps (or Power & battery) › Memory:** the protected list,
   whether the watcher may close apps itself (on by default), and a short
   history of what it closed.

## Steps

1. Apps in their own scopes (compositor spawn, launcher, dock, Files).
2. oom_score_adj and ManagedOOMPreference per app; the protected list
   setting.
3. The watcher: PSI, per-app memory, leak suspicion, the island warning.
4. Forced closes and their crash records.
5. The Settings page.
