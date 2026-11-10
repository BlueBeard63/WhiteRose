//! wlr-foreign-toplevel-management-v1: tells clients (QuickShell) about every
//! window, so the dock and switchers get handles they can activate, close or
//! capture. Each manager gets one handle per window; title, app id and state
//! changes are sent only when they actually change.

use smithay::{
    desktop::Window,
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel::State as ToplevelState,
        wayland_protocols_wlr::foreign_toplevel::v1::server::{
            zwlr_foreign_toplevel_handle_v1::{self, ZwlrForeignToplevelHandleV1},
            zwlr_foreign_toplevel_manager_v1::{self, ZwlrForeignToplevelManagerV1},
        },
        wayland_server::{Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource},
    },
};

use crate::state::WhiteRose;

const VERSION: u32 = 3;

/// What a handle last told its client.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ToplevelInfo {
    pub title: String,
    pub app_id: String,
    pub activated: bool,
    pub maximized: bool,
    pub fullscreen: bool,
}

impl ToplevelInfo {
    /// The protocol's state array: little-endian u32s.
    pub fn state_bytes(&self) -> Vec<u8> {
        use zwlr_foreign_toplevel_handle_v1::State;
        let mut states = Vec::new();
        if self.maximized {
            states.push(State::Maximized as u32);
        }
        if self.activated {
            states.push(State::Activated as u32);
        }
        if self.fullscreen {
            states.push(State::Fullscreen as u32);
        }
        states.iter().flat_map(|s| s.to_ne_bytes()).collect()
    }
}

struct Entry {
    window: u64,
    handles: Vec<ZwlrForeignToplevelHandleV1>,
    sent: Option<ToplevelInfo>,
}

#[derive(Default)]
pub struct ForeignToplevelState {
    managers: Vec<ZwlrForeignToplevelManagerV1>,
    entries: Vec<Entry>,
}

impl ForeignToplevelState {
    pub fn new(dh: &DisplayHandle) -> Self {
        dh.create_global::<WhiteRose, ZwlrForeignToplevelManagerV1, ()>(VERSION, ());
        Self::default()
    }

    pub fn handle_count(&self) -> usize {
        self.entries.iter().map(|e| e.handles.len()).sum()
    }
}

impl WhiteRose {
    fn toplevel_info(&self, w: &Window, focused: Option<&Window>) -> ToplevelInfo {
        let (app_id, title) = crate::ipc::toplevel_ids(w);
        let has = |s: ToplevelState| w.toplevel().is_some_and(|t| t.with_pending_state(|p| p.states.contains(s)));
        ToplevelInfo {
            title: title.unwrap_or_default(),
            app_id: app_id.unwrap_or_default(),
            activated: Some(w) == focused,
            maximized: has(ToplevelState::Maximized),
            fullscreen: has(ToplevelState::Fullscreen),
        }
    }

    fn new_handle(&self, manager: &ZwlrForeignToplevelManagerV1, window: u64) -> Option<ZwlrForeignToplevelHandleV1> {
        let client = manager.client()?;
        let handle = client
            .create_resource::<ZwlrForeignToplevelHandleV1, u64, WhiteRose>(
                &self.display_handle,
                manager.version(),
                window,
            )
            .ok()?;
        manager.toplevel(&handle);
        Some(handle)
    }

    /// Brings every client's handles in line with the current windows. Cheap
    /// when nothing changed; called whenever windows, titles or focus change.
    pub fn foreign_toplevel_refresh(&mut self) {
        let focused = self.focused_window();
        let current: Vec<(u64, ToplevelInfo)> = self
            .workspaces
            .mru()
            .iter()
            .map(|w| (self.window_id(w), self.toplevel_info(w, focused.as_ref())))
            .collect();

        // Closed windows.
        let mut ft = std::mem::take(&mut self.foreign_toplevels);
        ft.entries.retain(|e| {
            let alive = current.iter().any(|(id, _)| *id == e.window);
            if !alive {
                for h in &e.handles {
                    h.closed();
                }
            }
            alive
        });
        // New windows: a handle for every manager.
        for (id, _) in &current {
            if !ft.entries.iter().any(|e| e.window == *id) {
                let handles = ft.managers.iter().filter_map(|m| self.new_handle(m, *id)).collect();
                ft.entries.push(Entry { window: *id, handles, sent: None });
            }
        }
        // Changes.
        for e in &mut ft.entries {
            let Some((_, info)) = current.iter().find(|(id, _)| *id == e.window) else { continue };
            if e.sent.as_ref() == Some(info) {
                continue;
            }
            for h in &e.handles {
                send_info(h, e.sent.as_ref(), info);
            }
            e.sent = Some(info.clone());
        }
        self.foreign_toplevels = ft;
    }

    fn window_by_id(&self, id: u64) -> Option<Window> {
        self.workspaces.mru().iter().find(|w| self.window_id(w) == id).cloned()
    }
}

fn send_info(h: &ZwlrForeignToplevelHandleV1, old: Option<&ToplevelInfo>, new: &ToplevelInfo) {
    if old.is_none_or(|o| o.title != new.title) {
        h.title(new.title.clone());
    }
    if old.is_none_or(|o| o.app_id != new.app_id) {
        h.app_id(new.app_id.clone());
    }
    h.state(new.state_bytes());
    h.done();
}

impl GlobalDispatch<ZwlrForeignToplevelManagerV1, ()> for WhiteRose {
    fn bind(
        state: &mut Self,
        _dh: &DisplayHandle,
        _client: &Client,
        resource: New<ZwlrForeignToplevelManagerV1>,
        _: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        let manager = data_init.init(resource, ());
        // Announce existing windows to the new manager in creation (id) order,
        // the same order later windows arrive in, so a client's list of
        // handles lines up with window ids.
        let focused = state.focused_window();
        let mut windows: Vec<(u64, ToplevelInfo)> = state
            .workspaces
            .mru()
            .iter()
            .map(|w| (state.window_id(w), state.toplevel_info(w, focused.as_ref())))
            .collect();
        windows.sort_by_key(|(id, _)| *id);
        for (id, info) in windows {
            if let Some(h) = state.new_handle(&manager, id) {
                send_info(&h, None, &info);
                match state.foreign_toplevels.entries.iter_mut().find(|e| e.window == id) {
                    Some(e) => e.handles.push(h),
                    None => {
                        state.foreign_toplevels.entries.push(Entry { window: id, handles: vec![h], sent: Some(info) })
                    }
                }
            }
        }
        state.foreign_toplevels.managers.push(manager);
    }
}

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for WhiteRose {
    fn request(
        state: &mut Self,
        _client: &Client,
        manager: &ZwlrForeignToplevelManagerV1,
        request: zwlr_foreign_toplevel_manager_v1::Request,
        _: &(),
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        if let zwlr_foreign_toplevel_manager_v1::Request::Stop = request {
            state.foreign_toplevels.managers.retain(|m| m != manager);
            manager.finished();
        }
    }

    fn destroyed(
        state: &mut Self,
        _client: smithay::reexports::wayland_server::backend::ClientId,
        manager: &ZwlrForeignToplevelManagerV1,
        _: &(),
    ) {
        state.foreign_toplevels.managers.retain(|m| m != manager);
    }
}

impl Dispatch<ZwlrForeignToplevelHandleV1, u64> for WhiteRose {
    fn request(
        state: &mut Self,
        _client: &Client,
        _handle: &ZwlrForeignToplevelHandleV1,
        request: zwlr_foreign_toplevel_handle_v1::Request,
        window: &u64,
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        use zwlr_foreign_toplevel_handle_v1::Request;
        let Some(w) = state.window_by_id(*window) else { return };
        match request {
            Request::Activate { .. } => state.activate_window(&w),
            Request::Close => {
                if let Some(t) = w.toplevel() {
                    t.send_close();
                }
            }
            Request::SetMaximized => state.maximize(&w),
            Request::UnsetMaximized => state.unmaximize(&w),
            Request::SetFullscreen { output } => {
                state.fullscreen_to(&w, output.as_ref().and_then(smithay::output::Output::from_resource))
            }
            Request::UnsetFullscreen => state.unfullscreen(&w),
            // WhiteRose has no minimized state; windows live on workspaces instead.
            Request::SetMinimized | Request::UnsetMinimized | Request::SetRectangle { .. } => {}
            Request::Destroy => {}
            _ => {}
        }
        state.foreign_toplevel_refresh();
    }

    fn destroyed(
        state: &mut Self,
        _client: smithay::reexports::wayland_server::backend::ClientId,
        handle: &ZwlrForeignToplevelHandleV1,
        _: &u64,
    ) {
        for e in &mut state.foreign_toplevels.entries {
            e.handles.retain(|h| h != handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_state_array() {
        let info = ToplevelInfo { activated: true, fullscreen: true, ..Default::default() };
        let words: Vec<u32> = info.state_bytes().as_chunks::<4>().0.iter().map(|c| u32::from_ne_bytes(*c)).collect();
        assert_eq!(words, vec![2, 3], "activated = 2, fullscreen = 3");
        assert!(ToplevelInfo::default().state_bytes().is_empty());
    }
}
