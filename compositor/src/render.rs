//! Frame composition shared by every backend and by screenshots, so a
//! capture is exactly what was on screen.

use std::time::Duration;

use smithay::{
    backend::renderer::{
        damage::OutputDamageTracker,
        element::{surface::WaylandSurfaceRenderElement, AsRenderElements},
        gles::{GlesRenderer, GlesTarget},
    },
    desktop::layer_map_for_output,
    output::Output,
    utils::{Physical, Rectangle, Scale},
    wayland::shell::wlr_layer::Layer as WlrLayer,
};

use crate::state::WhiteRose;

/// Background of the empty desktop: white, as in the Figma frames, so the
/// black bar and island stand out. Replaced by the wallpaper later.
pub const CLEAR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

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
    let result = match state.active_fullscreen() {
        Some(window) => {
            let scale = output.current_scale().fractional_scale();
            // Elements are listed front to back.
            let mut elements: Vec<WaylandSurfaceRenderElement<GlesRenderer>> = Vec::new();
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
            &[],
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
    for window in state.space.elements() {
        if fullscreen.as_ref().is_none_or(|f| f == window) {
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
