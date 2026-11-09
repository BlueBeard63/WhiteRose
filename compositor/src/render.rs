//! Frame composition shared by every backend and by screenshots, so a
//! capture is exactly what was on screen.

use std::time::Duration;

use smithay::{
    backend::renderer::{
        damage::OutputDamageTracker,
        element::{
            surface::{render_elements_from_surface_tree, WaylandSurfaceRenderElement},
            AsRenderElements, Kind,
        },
        gles::{GlesRenderer, GlesTarget},
    },
    desktop::layer_map_for_output,
    output::Output,
    utils::{Physical, Rectangle, Scale},
    wayland::shell::wlr_layer::Layer as WlrLayer,
};

use smithay::reexports::wayland_server::Resource;

use crate::state::WhiteRose;

/// Background of the empty desktop: white, as in the Figma frames, so the
/// black bar and island stand out. Replaced by the wallpaper later.
pub const CLEAR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

/// Behind the lock screen (and instead of it, until it has drawn): black.
pub const LOCKED: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// Draws one frame of `output` into `framebuffer`. Returns the damaged
/// regions, or None if nothing changed since the tracker's last frame.
///
/// Normally this is the whole space (windows plus layer surfaces in order).
/// With a fullscreen window it is only that window under the overlay layer,
/// which skips the bar and everything behind it.
pub fn compose(
    renderer: &mut GlesRenderer,
    framebuffer: &mut GlesTarget<'_>,
    output: &Output,
    state: &WhiteRose,
    tracker: &mut OutputDamageTracker,
    age: usize,
) -> Result<Option<Vec<Rectangle<i32, Physical>>>, String> {
    let scale = output.current_scale().fractional_scale();

    // Locked: the lock surface and nothing else.
    if state.is_locked() {
        let elements: Vec<WaylandSurfaceRenderElement<GlesRenderer>> = match state.lock.surface_for(output) {
            Some(lock) => render_elements_from_surface_tree(
                renderer,
                lock.wl_surface(),
                (0, 0),
                Scale::from(scale),
                1.0,
                Kind::Unspecified,
            ),
            None => Vec::new(),
        };
        return tracker
            .render_output(renderer, framebuffer, age, &elements, LOCKED)
            .map(|r| r.damage.cloned())
            .map_err(|e| format!("{e:?}"));
    }

    // A drag in progress: its icon follows the pointer, above everything.
    let mut overlay: Vec<WaylandSurfaceRenderElement<GlesRenderer>> = Vec::new();
    if let Some(icon) = state.dnd_icon.as_ref().filter(|i| i.is_alive()) {
        let origin = state.space.output_geometry(output).map(|g| g.loc).unwrap_or_default().to_f64();
        let at = state.seat.get_pointer().map(|p| p.current_location()).unwrap_or_default() - origin;
        overlay = render_elements_from_surface_tree(
            renderer,
            icon,
            at.to_physical_precise_round(scale),
            Scale::from(scale),
            1.0,
            Kind::Unspecified,
        );
    }

    let result = match state.active_fullscreen() {
        Some(window) => {
            // Elements are listed front to back.
            let mut elements: Vec<WaylandSurfaceRenderElement<GlesRenderer>> = overlay;
            {
                let map = layer_map_for_output(output);
                for layer in map.layers_on(WlrLayer::Overlay) {
                    let loc = map.layer_geometry(layer).map(|g| g.loc).unwrap_or_default();
                    elements.extend(layer.render_elements(
                        renderer,
                        loc.to_physical_precise_round(scale),
                        Scale::from(scale),
                        1.0,
                    ));
                }
            }
            let loc = state.space.element_location(window).unwrap_or_default() - window.geometry().loc;
            elements.extend(window.render_elements(
                renderer,
                loc.to_physical_precise_round(scale),
                Scale::from(scale),
                1.0,
            ));
            tracker.render_output(renderer, framebuffer, age, &elements, CLEAR).map(|r| r.damage.cloned())
        }
        None => smithay::desktop::space::render_output::<_, WaylandSurfaceRenderElement<GlesRenderer>, _, _>(
            output,
            renderer,
            framebuffer,
            1.0,
            age,
            [&state.space],
            &overlay,
            tracker,
            CLEAR,
        )
        .map(|r| r.damage.cloned()),
    };
    result.map_err(|e| format!("{e:?}"))
}

/// Tells clients that drew this frame they may draw the next one. Only
/// visible surfaces get a callback, so hidden ones (the bar under a fullscreen
/// video, windows behind it) stop drawing entirely.
pub fn send_frame_callbacks(output: &Output, state: &mut WhiteRose) {
    let fullscreen = state.active_fullscreen().cloned();
    let elapsed = state.start_time.elapsed();
    // Locked: only the lock screen draws; everything else waits, unseen.
    if state.is_locked() {
        for lock in state.lock.surfaces() {
            smithay::desktop::utils::send_frames_surface_tree(
                lock.wl_surface(),
                output,
                elapsed,
                Some(Duration::ZERO),
                |_, _| Some(output.clone()),
            );
        }
        let _ = state.display_handle.flush_clients();
        return;
    }
    for window in state.space.elements() {
        if fullscreen.as_ref().is_none_or(|f| f == window) {
            window.send_frame(output, elapsed, Some(Duration::ZERO), |_, _| Some(output.clone()));
        }
    }
    // Windows being previewed keep drawing even if hidden on another workspace.
    for window in state.exporting_windows() {
        if state.space.element_location(&window).is_none() {
            window.send_frame(output, elapsed, Some(Duration::ZERO), |_, _| Some(output.clone()));
        }
    }
    {
        let mut layers = layer_map_for_output(output);
        for layer in layers.layers() {
            if fullscreen.is_none() || layer.layer() == WlrLayer::Overlay {
                layer.send_frame(output, elapsed, Some(Duration::ZERO), |_, _| Some(output.clone()));
            }
        }
        layers.cleanup();
    }
    state.space.refresh();
    state.popups.cleanup();
    let _ = state.display_handle.flush_clients();
}
