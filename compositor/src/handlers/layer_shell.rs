//! wlr-layer-shell: how QuickShell draws the bar, panels, launcher and
//! screenshot overlay. Based on Smithay's anvil compositor (MIT).

use smithay::{
    delegate_layer_shell,
    desktop::{layer_map_for_output, LayerSurface, WindowSurfaceType},
    output::Output,
    reexports::wayland_server::protocol::{wl_output::WlOutput, wl_surface::WlSurface},
    utils::SERIAL_COUNTER,
    wayland::{
        compositor::with_states,
        shell::wlr_layer::{
            KeyboardInteractivity, Layer, LayerSurface as WlrLayerSurface, LayerSurfaceData, WlrLayerShellHandler,
            WlrLayerShellState,
        },
    },
};

use std::cell::Cell;

use crate::state::WhiteRose;

/// Whether a layer surface's current exclusive-keyboard request was granted.
struct ExclusiveGrant(Cell<bool>);

impl WlrLayerShellHandler for WhiteRose {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: WlrLayerSurface,
        wl_output: Option<WlOutput>,
        _layer: Layer,
        namespace: String,
    ) {
        let output =
            wl_output.as_ref().and_then(Output::from_resource).or_else(|| self.space.outputs().next().cloned());
        let Some(output) = output else {
            // Nowhere to show it; tell the client instead of leaving it waiting.
            surface.send_close();
            return;
        };
        let mut map = layer_map_for_output(&output);
        match map.map_layer(&LayerSurface::new(surface, namespace.clone())) {
            Ok(()) => tracing::info!(namespace, output = output.name(), "layer surface opened"),
            Err(err) => tracing::warn!(namespace, ?err, "failed to map layer surface"),
        }
    }

    fn layer_destroyed(&mut self, surface: WlrLayerSurface) {
        let had_focus = self.keyboard_focus().as_ref() == Some(surface.wl_surface());
        for output in self.space.outputs() {
            let mut map = layer_map_for_output(output);
            let found = map.layers().find(|l| l.layer_surface() == &surface).cloned();
            if let Some(layer) = found {
                map.unmap_layer(&layer);
                map.arrange();
            }
        }
        if had_focus {
            // e.g. the launcher closed: give focus back to the window underneath.
            let window = self.focused_window();
            self.focus_window(window);
        }
        self.schedule_redraw();
    }
}
delegate_layer_shell!(WhiteRose);

impl WhiteRose {
    /// Handles a commit on a layer surface. Returns false if `surface` is not one.
    pub(crate) fn layer_commit(&mut self, surface: &WlSurface) -> bool {
        let Some(output) = self
            .space
            .outputs()
            .find(|o| layer_map_for_output(o).layer_for_surface(surface, WindowSurfaceType::TOPLEVEL).is_some())
            .cloned()
        else {
            return false;
        };

        let initial_configure_sent = with_states(surface, |states| {
            states.data_map.get::<LayerSurfaceData>().is_some_and(|d| d.lock().unwrap().initial_configure_sent)
        });

        let layer = {
            let mut map = layer_map_for_output(&output);
            // Re-arrange on every commit: size, anchors and exclusive zone may have changed.
            map.arrange();
            map.layer_for_surface(surface, WindowSurfaceType::TOPLEVEL).cloned()
        };
        let Some(layer) = layer else { return true };

        if !initial_configure_sent {
            layer.layer_surface().send_configure();
        }

        // Panels that open (dock, launcher, screenshot overlay) ask for exclusive
        // keyboard focus. Grant it once, when the request starts: re-granting on
        // every commit would steal focus back from a window the user clicked,
        // because the bar commits constantly (clock, EQ). When the request ends,
        // hand the keyboard back to the window.
        let state = layer.cached_state();
        let wants_exclusive = state.keyboard_interactivity == KeyboardInteractivity::Exclusive
            && matches!(state.layer, Layer::Top | Layer::Overlay);
        let granted = layer.user_data().get_or_insert(|| ExclusiveGrant(Cell::new(false)));
        if wants_exclusive && !granted.0.get() {
            granted.0.set(true);
            let keyboard = self.seat.get_keyboard().expect("seat has a keyboard");
            keyboard.set_focus(self, Some(surface.clone()), SERIAL_COUNTER.next_serial());
        } else if !wants_exclusive && granted.0.replace(false) && self.keyboard_focus().as_ref() == Some(surface) {
            let window = self.focused_window();
            self.focus_window(window);
        }
        true
    }
}
