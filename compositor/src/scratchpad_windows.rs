//! Scratchpads (`scratchpad.rs`) in the compositor: their keys, claiming
//! the window a key started, and showing and hiding it.
//!
//! A scratchpad's window isn't on any workspace (it's kept out of
//! `Workspaces`), so switching workspace leaves it be, and the switchers,
//! the dock and the shell's window list don't see it. Shown, it floats
//! above every other window, fullscreen ones included, on the monitor the
//! pointer was on when its key was pressed.

use std::time::Instant;

use serde_json::Value;
use smithay::{
    desktop::Window,
    output::Output,
    reexports::wayland_server::Resource,
    utils::{Logical, Point},
};

use crate::{
    layout::Rect,
    scratchpad::{self, Pad, State, Step},
    state::WhiteRose,
};

/// The scratchpads from the setting, and where each one is.
#[derive(Debug, Default)]
pub struct Scratchpads {
    pub pads: Vec<Pad>,
    /// By pad id; a pad not here is `Off`.
    states: Vec<(String, State<Window, Output>)>,
    /// The scratchpad window that has the keyboard, if one does: the
    /// "focused window" then, since it isn't on a workspace.
    pub focus: Option<Window>,
}

impl Scratchpads {
    fn state(&self, id: &str) -> Option<&State<Window, Output>> {
        self.states.iter().find(|(i, _)| i == id).map(|(_, s)| s)
    }

    fn set(&mut self, id: &str, state: State<Window, Output>) {
        self.states.retain(|(i, _)| i != id);
        if state != State::Off {
            self.states.push((id.to_owned(), state));
        }
    }

    /// The id of the pad whose window `w` is.
    pub fn pad_of(&self, w: &Window) -> Option<&str> {
        self.states.iter().find(|(_, s)| s.window() == Some(w)).map(|(i, _)| i.as_str())
    }

    pub fn is_pad(&self, w: &Window) -> bool {
        self.pad_of(w).is_some()
    }

    /// Whether `w` is a scratchpad that's showing.
    pub fn is_shown(&self, w: &Window) -> bool {
        self.states.iter().any(|(_, s)| matches!(s, State::Shown { window, .. } if window == w))
    }

    /// The scratchpad windows showing on `output`, bottom to top as they
    /// were shown.
    pub fn shown_on<'a>(&'a self, output: &'a Output) -> impl Iterator<Item = &'a Window> + 'a {
        self.states.iter().filter_map(move |(_, s)| match s {
            State::Shown { window, on } if on == output => Some(window),
            _ => None,
        })
    }

    fn pad(&self, id: &str) -> Option<&Pad> {
        self.pads.iter().find(|p| p.id == id)
    }
}

impl WhiteRose {
    /// Takes the `windows.scratchpads` setting; true if the shortcut table
    /// must be rebuilt (it always is: the keys may have changed). A pad
    /// that's gone lets its window go: it becomes an ordinary window on the
    /// active workspace.
    pub fn scratchpads_setting(&mut self, key: &str, value: &Value) -> bool {
        if key != "windows.scratchpads" {
            return false;
        }
        self.scratchpads.pads = scratchpad::parse(value);
        let gone: Vec<(String, State<Window, Output>)> = {
            let pads = &self.scratchpads.pads;
            let (keep, gone) = std::mem::take(&mut self.scratchpads.states)
                .into_iter()
                .partition(|(id, _)| pads.iter().any(|p| &p.id == id));
            self.scratchpads.states = keep;
            gone
        };
        for w in gone.iter().filter_map(|(_, s)| s.window()) {
            if self.scratchpads.focus.as_ref() == Some(w) {
                self.scratchpads.focus = None;
            }
            if self.space.element_location(w).is_none() {
                self.show(w);
            }
            self.workspaces.add(w.clone());
            self.focus_window(Some(w.clone()));
        }
        true
    }

    /// A scratchpad's key: start it, show it here, focus it or hide it.
    pub fn scratchpad_key(&mut self, id: &str) {
        let Some(pad) = self.scratchpads.pad(id).cloned() else { return };
        let Some(here) = self.pointer_output() else { return };
        let state = self.scratchpads.state(id).cloned().unwrap_or_default();
        let focused = state.window().is_some_and(|w| self.has_keyboard(w));
        match state.step(&here, focused, Instant::now()) {
            Step::Start => match self.spawn(&pad.command) {
                Some(pid) => self.expect_scratchpad(id, pid),
                None => self.scratchpads.set(id, State::Off),
            },
            Step::Wait => {}
            Step::Show | Step::MoveHere => self.show_scratchpad(id, &here),
            Step::Focus => self.focus_window(state.window().cloned()),
            Step::Hide => self.hide_scratchpad(id),
        }
    }

    /// Waits for the first window of process `pid` (or a child of it) to
    /// make it pad `id`'s.
    pub fn expect_scratchpad(&mut self, id: &str, pid: u32) {
        self.scratchpads.set(id, State::Starting { pid, since: Instant::now() });
    }

    /// On a window's first commit: if a pad is waiting for this client's
    /// process, the window becomes that pad's and shows. True if it was
    /// claimed (the window rules' placement then doesn't apply).
    pub(crate) fn claim_scratchpad(&mut self, w: &Window) -> bool {
        let Some(pid) = self.client_pid(w) else { return false };
        let now = Instant::now();
        let Some(id) = self
            .scratchpads
            .states
            .iter()
            .find(|(_, s)| s.claims(pid, now, scratchpad::proc_parent))
            .map(|(i, _)| i.clone())
        else {
            return false;
        };
        tracing::info!(scratchpad = id, "scratchpad window opened");
        // Off its workspace: it was added there when it was created.
        self.workspaces.remove(w);
        if !self.tiling.floating.contains(w) {
            self.tiling.floating.push(w.clone());
        }
        self.scratchpads.set(&id, State::Hidden(w.clone()));
        let here = self.pointer_output().or_else(|| self.output_of(w));
        match here {
            Some(o) => self.show_scratchpad(&id, &o),
            None => self.focus_window(Some(w.clone())),
        }
        true
    }

    /// Shows pad `id`'s window on `output` (moving it there if it's shown
    /// elsewhere), above everything, and focuses it. Its size and place
    /// are worked out afresh each time: the bar or the monitor may have
    /// changed while it was hidden.
    fn show_scratchpad(&mut self, id: &str, output: &Output) {
        let Some(pad) = self.scratchpads.pad(id).cloned() else { return };
        let Some(w) = self.scratchpads.state(id).and_then(State::window).cloned() else { return };
        let usable = self.usable_area_on(output);
        let area = Rect::new(usable.loc.x, usable.loc.y, usable.size.w, usable.size.h);
        let rect = pad.rect(area);
        // The size it has drawn at, if it kept the one it was given (an
        // app with a minimum may not): placed by that, else by the rect.
        let drawn = w.geometry().size;
        if let Some(t) = w.toplevel() {
            t.with_pending_state(|s| s.size = Some((rect.w, rect.h).into()));
            t.send_pending_configure();
        }
        let (x, y) = if drawn.w > 0 && drawn.h > 0 { pad.origin(area, (drawn.w, drawn.h)) } else { (rect.x, rect.y) };
        self.hidden_at.retain(|(h, _)| h != &w);
        self.space.map_element(w.clone(), Point::<i32, Logical>::from((x, y)), true);
        self.scratchpads.set(id, State::Shown { window: w.clone(), on: output.clone() });
        self.focus_window(Some(w));
    }

    /// Hides pad `id`'s window; if it had the keyboard, focus goes back to
    /// the workspace's window.
    fn hide_scratchpad(&mut self, id: &str) {
        let Some(w) = self.scratchpads.state(id).and_then(State::window).cloned() else { return };
        let had_focus = self.scratchpads.focus.as_ref() == Some(&w);
        self.put_away(id, &w);
        if had_focus {
            let next = self.next_focus();
            self.focus_window(next);
        } else {
            self.schedule_redraw();
        }
    }

    /// Unmaps a pad's window and marks it hidden, leaving focus alone.
    fn put_away(&mut self, id: &str, w: &Window) {
        if self.scratchpads.focus.as_ref() == Some(w) {
            self.scratchpads.focus = None;
        }
        w.set_activated(false);
        if let Some(t) = w.toplevel() {
            t.send_pending_configure();
        }
        self.hide(w);
        self.scratchpads.set(id, State::Hidden(w.clone()));
    }

    /// Focus is moving to `to`: remembers whether that's a scratchpad, and
    /// hides the one losing it if it hides on losing focus. Called by
    /// `focus_window` before it moves the keyboard.
    pub(crate) fn scratchpad_focus_moves(&mut self, to: Option<&Window>) {
        let from = self.scratchpads.focus.take();
        if let Some(w) = to.filter(|w| self.scratchpads.is_shown(w)) {
            self.scratchpads.focus = Some(w.clone());
        }
        let Some(from) = from.filter(|f| Some(f) != to) else { return };
        let Some(id) = self.scratchpads.pad_of(&from).map(str::to_owned) else { return };
        if self.scratchpads.pad(&id).is_some_and(|p| p.hide_on_blur) && self.scratchpads.is_shown(&from) {
            self.put_away(&id, &from);
        }
    }

    /// A closed window: if it was a pad's, the pad is off (its key starts
    /// it again).
    pub(crate) fn forget_scratchpad(&mut self, w: &Window) {
        if self.scratchpads.focus.as_ref() == Some(w) {
            self.scratchpads.focus = None;
        }
        if let Some(id) = self.scratchpads.pad_of(w).map(str::to_owned) {
            self.scratchpads.set(&id, State::Off);
        }
    }

    /// Asked to minimize: a pad hides instead. True if `w` is a pad's.
    pub(crate) fn minimize_scratchpad(&mut self, w: &Window) -> bool {
        let Some(id) = self.scratchpads.pad_of(w).map(str::to_owned) else { return false };
        self.hide_scratchpad(&id);
        true
    }

    /// Whether `w` has the keyboard.
    fn has_keyboard(&self, w: &Window) -> bool {
        self.keyboard_focus().zip(w.toplevel()).is_some_and(|(f, t)| &f == t.wl_surface())
    }

    /// The process id of the client that owns `w`.
    fn client_pid(&self, w: &Window) -> Option<u32> {
        let client = w.toplevel()?.wl_surface().client()?;
        let creds = client.get_credentials(&self.display_handle).ok()?;
        u32::try_from(creds.pid).ok()
    }
}
