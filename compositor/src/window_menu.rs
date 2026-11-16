//! The window menu (Figma "Window menu - right-click on a title bar"): an
//! app asks for it when its own title bar is right-clicked (xdg_toplevel
//! show_window_menu), or ALT+SPACE opens it for the focused window. The
//! shell draws it (WindowMenu.qml) from the `window_menu` event: move the
//! window to a workspace, put it on another activity or every activity,
//! close it. It closes with Esc, a click elsewhere, after its action, or
//! when the window goes.

use smithay::{
    desktop::Window,
    utils::{Logical, Point},
};

use crate::{ipc::WindowMenuView, state::WhiteRose};

impl WhiteRose {
    /// Opens the menu for `w` at `at` (global).
    pub fn open_window_menu(&mut self, w: &Window, at: Point<i32, Logical>) {
        if self.is_locked() || self.workspaces.workspace_of(w).is_none() {
            return;
        }
        self.window_menu = Some((w.clone(), at));
        self.window_menu_sync();
    }

    pub fn close_window_menu(&mut self) {
        if self.window_menu.take().is_some() {
            self.window_menu_sync();
        }
    }

    /// What the shell's told.
    pub fn window_menu_event(&self) -> crate::ipc::Event {
        crate::ipc::Event::WindowMenu {
            menu: self.window_menu.as_ref().map(|(w, at)| WindowMenuView { id: self.window_id(w), x: at.x, y: at.y }),
        }
    }

    fn window_menu_sync(&mut self) {
        let e = self.window_menu_event();
        if let Some(server) = self.ipc.as_mut() {
            server.update(e);
        }
    }
}
