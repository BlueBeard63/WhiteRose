// Adapted from Smithay's smallvil example (MIT, (c) Victor Berger and Victoria Brekenfeld).
use crate::{
    grabs::resize_grab,
    state::{ClientState, WhiteRose},
};
use smithay::{
    backend::renderer::utils::on_commit_buffer_handler,
    delegate_compositor, delegate_shm,
    reexports::wayland_server::{
        protocol::{wl_buffer, wl_surface::WlSurface},
        Client,
    },
    wayland::{
        buffer::BufferHandler,
        compositor::{get_parent, is_sync_subsurface, CompositorClientState, CompositorHandler, CompositorState},
        shm::{ShmHandler, ShmState},
    },
};

use super::xdg_shell;

impl CompositorHandler for WhiteRose {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client.get_data::<ClientState>().unwrap().compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);
        if self.layer_commit(surface) {
            self.refit_windows();
            self.schedule_redraw();
            return;
        }
        if !is_sync_subsurface(surface) {
            let mut root = surface.clone();
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }
            // Includes windows hidden on other workspaces, so their previews update too.
            if let Some(window) = self.window_for_surface(&root) {
                window.on_commit();
                self.mark_export_dirty(&window);
                // Its size may have changed (maximized, resized).
                self.island_sync();
            }
        };

        xdg_shell::handle_commit(&mut self.popups, &self.space, surface);
        resize_grab::handle_commit(&mut self.space, surface);
        self.schedule_redraw();
    }
}

impl BufferHandler for WhiteRose {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl ShmHandler for WhiteRose {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

delegate_compositor!(WhiteRose);
delegate_shm!(WhiteRose);
