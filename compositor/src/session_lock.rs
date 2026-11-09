//! Screen locking (ext-session-lock-v1). The shell's lock screen asks to lock;
//! from then on the compositor draws nothing but the lock surfaces (black
//! where an output has none yet), sends all input to them, runs no
//! shortcuts, and refuses window previews. Only the locking client's unlock
//! ends it. If that client dies, the screen stays locked until a new lock
//! client (the restarted shell) takes over and unlocks.

use smithay::{
    delegate_session_lock,
    desktop::{utils::under_from_surface_tree, WindowSurfaceType},
    output::Output,
    reexports::{
        wayland_protocols::ext::session_lock::v1::server::ext_session_lock_v1::ExtSessionLockV1,
        wayland_server::protocol::{wl_output::WlOutput, wl_surface::WlSurface},
    },
    utils::{Logical, Point, SERIAL_COUNTER},
    wayland::session_lock::{LockSurface, SessionLockHandler, SessionLockManagerState, SessionLocker},
};

use crate::{ipc, state::WhiteRose};

#[derive(Default)]
pub struct SessionLock {
    /// True from the lock request until the unlock: nothing else is drawn.
    pub locked: bool,
    /// Confirmed to the client once a frame without the desktop was shown.
    pending: Option<SessionLocker>,
    /// The lock object in charge (a newer lock replaces a dead client's).
    current: Option<ExtSessionLockV1>,
    surfaces: Vec<(Output, LockSurface)>,
}

impl SessionLock {
    pub fn surface_for(&self, output: &Output) -> Option<&LockSurface> {
        self.surfaces.iter().find(|(o, s)| o == output && s.alive()).map(|(_, s)| s)
    }

    pub fn surfaces(&self) -> impl Iterator<Item = &LockSurface> {
        self.surfaces.iter().map(|(_, s)| s).filter(|s| s.alive())
    }
}

impl WhiteRose {
    pub fn is_locked(&self) -> bool {
        self.lock.locked
    }

    /// Call after a frame has been presented: a lock is only confirmed to the
    /// client once the screen really stopped showing the desktop.
    pub fn frame_presented(&mut self) {
        if let Some(locker) = self.lock.pending.take() {
            locker.lock();
            tracing::info!("session locked");
        }
    }

    /// The lock surface under `pos` while locked.
    pub fn lock_surface_under(&self, pos: Point<f64, Logical>) -> Option<(WlSurface, Point<f64, Logical>)> {
        let (output, geo) = self
            .space
            .outputs()
            .find_map(|o| self.space.output_geometry(o).filter(|g| g.to_f64().contains(pos)).map(|g| (o, g)))?;
        let lock = self.lock.surface_for(output)?;
        under_from_surface_tree(lock.wl_surface(), pos - geo.loc.to_f64(), geo.loc, WindowSurfaceType::ALL)
            .map(|(s, p)| (s, p.to_f64()))
    }

    /// Keyboard to the lock surface (the one under the pointer, else the first).
    pub fn focus_lock_surface(&mut self) {
        let at = self.seat.get_pointer().map(|p| p.current_location()).unwrap_or_default();
        let surface = self
            .space
            .outputs()
            .find(|o| self.space.output_geometry(o).is_some_and(|g| g.to_f64().contains(at)))
            .and_then(|o| self.lock.surface_for(o))
            .or_else(|| self.lock.surfaces().next())
            .map(|s| s.wl_surface().clone());
        let keyboard = self.seat.get_keyboard().expect("seat has a keyboard");
        keyboard.set_focus(self, surface, SERIAL_COUNTER.next_serial());
    }

    fn lock_sync(&mut self) {
        let locked = self.lock.locked;
        if let Some(ipc) = self.ipc.as_mut() {
            ipc.update(ipc::Event::Lock { locked });
        }
    }
}

impl SessionLockHandler for WhiteRose {
    fn lock_state(&mut self) -> &mut SessionLockManagerState {
        &mut self.session_lock_state
    }

    fn lock(&mut self, confirmation: SessionLocker) {
        // A new lock while already locked: the previous locker died (the
        // shell restarted). The new one takes over.
        self.lock.surfaces.clear();
        self.lock.locked = true;
        self.lock.current = Some(confirmation.ext_session_lock().clone());
        self.lock.pending = Some(confirmation);
        self.dnd_icon = None;
        self.switcher = None;
        self.pending_exports.drain(..).for_each(crate::toplevel_export::PendingCopy::fail);
        let keyboard = self.seat.get_keyboard().expect("seat has a keyboard");
        keyboard.set_focus(self, None, SERIAL_COUNTER.next_serial());
        self.lock_sync();
        self.schedule_redraw();
    }

    fn unlock(&mut self) {
        tracing::info!("session unlocked");
        self.lock = SessionLock::default();
        self.lock_sync();
        let window = self.focused_window();
        self.focus_window(window);
        self.schedule_redraw();
    }

    fn unlock_requested(&mut self, lock: &ExtSessionLockV1) {
        if self.lock.current.as_ref() == Some(lock) {
            self.unlock();
        } else {
            tracing::warn!("ignoring unlock from a lock that was replaced");
        }
    }

    fn lock_cancelled(&mut self, lock: &ExtSessionLockV1) {
        // Only a lock that was never confirmed can be cancelled: the screen
        // never stopped showing the desktop, so nothing is given away.
        if self.lock.current.as_ref() == Some(lock) && self.lock.pending.is_some() {
            tracing::info!("lock cancelled before it took effect");
            self.unlock();
        }
    }

    fn new_surface(&mut self, surface: LockSurface, wl_output: WlOutput) {
        let Some(output) = Output::from_resource(&wl_output) else { return };
        let Some(geo) = self.space.output_geometry(&output) else { return };
        surface.with_pending_state(|s| s.size = Some((geo.size.w as u32, geo.size.h as u32).into()));
        surface.send_configure();
        self.lock.surfaces.retain(|(o, _)| o != &output);
        self.lock.surfaces.push((output, surface));
        self.focus_lock_surface();
        self.schedule_redraw();
    }
}

delegate_session_lock!(WhiteRose);
