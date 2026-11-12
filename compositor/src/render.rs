//! Frame composition shared by every backend and by screenshots, so a
//! capture is exactly what was on screen.

use std::time::Duration;

use smithay::{
    backend::renderer::{
        damage::OutputDamageTracker,
        element::{
            memory::MemoryRenderBufferRenderElement,
            solid::SolidColorRenderElement,
            surface::{render_elements_from_surface_tree, WaylandSurfaceRenderElement},
            AsRenderElements, Kind,
        },
        gles::{GlesRenderer, GlesTarget},
    },
    desktop::{layer_map_for_output, space::SpaceRenderElements},
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

smithay::backend::renderer::element::render_elements! {
    /// Everything that can appear on an output, for any backend.
    pub OutputElement<=GlesRenderer>;
    Space=SpaceRenderElements<GlesRenderer, WaylandSurfaceRenderElement<GlesRenderer>>,
    Surface=WaylandSurfaceRenderElement<GlesRenderer>,
    Cursor=MemoryRenderBufferRenderElement<GlesRenderer>,
    Solid=SolidColorRenderElement,
}

/// What `output` shows, front to back, and the colour behind it. `front`
/// goes on top of everything (the cursor, on backends that draw their own).
///
/// Normally this is the whole space (windows plus layer surfaces in order).
/// With a fullscreen window it is only that window under the overlay layer,
/// which skips the bar and everything behind it. Locked, it is only the lock
/// surface.
pub fn output_elements(
    renderer: &mut GlesRenderer,
    output: &Output,
    state: &WhiteRose,
    front: Vec<OutputElement>,
) -> (Vec<OutputElement>, [f32; 4]) {
    let scale = output.current_scale().fractional_scale();
    let mut elements = front;

    // Locked: the lock surface and nothing else.
    if state.is_locked() {
        if let Some(lock) = state.lock.surface_for(output) {
            elements.extend(
                render_elements_from_surface_tree::<_, WaylandSurfaceRenderElement<_>>(
                    renderer,
                    lock.wl_surface(),
                    (0, 0),
                    Scale::from(scale),
                    1.0,
                    Kind::Unspecified,
                )
                .into_iter()
                .map(OutputElement::Surface),
            );
        }
        return (elements, LOCKED);
    }

    // A drag in progress: its icon follows the pointer, above everything.
    if let Some(icon) = state.dnd_icon.as_ref().filter(|i| i.is_alive()) {
        let origin = state.space.output_geometry(output).map(|g| g.loc).unwrap_or_default().to_f64();
        let at = state.seat.get_pointer().map(|p| p.current_location()).unwrap_or_default() - origin;
        elements.extend(
            render_elements_from_surface_tree::<_, WaylandSurfaceRenderElement<_>>(
                renderer,
                icon,
                at.to_physical_precise_round(scale),
                Scale::from(scale),
                1.0,
                Kind::Unspecified,
            )
            .into_iter()
            .map(OutputElement::Surface),
        );
    }

    match state.fullscreen_on(output) {
        Some(window) => {
            {
                let map = layer_map_for_output(output);
                for layer in map.layers_on(WlrLayer::Overlay) {
                    let loc = map.layer_geometry(layer).map(|g| g.loc).unwrap_or_default();
                    elements.extend(
                        layer
                            .render_elements::<WaylandSurfaceRenderElement<_>>(
                                renderer,
                                loc.to_physical_precise_round(scale),
                                Scale::from(scale),
                                1.0,
                            )
                            .into_iter()
                            .map(OutputElement::Surface),
                    );
                }
            }
            let loc = state.space.element_location(window).unwrap_or_default() - window.geometry().loc;
            elements.extend(
                window
                    .render_elements::<WaylandSurfaceRenderElement<_>>(
                        renderer,
                        loc.to_physical_precise_round(scale),
                        Scale::from(scale),
                        1.0,
                    )
                    .into_iter()
                    .map(OutputElement::Surface),
            );
        }
        None => {
            // Where a dragged tile will land: above the windows.
            let out = state.space.output_geometry(output).unwrap_or_default();
            if let Some(r) = state.tiling.preview.rect.and_then(|r| r.intersection(out)) {
                let local = Rectangle::new(r.loc - out.loc, r.size);
                elements.push(OutputElement::Solid(SolidColorRenderElement::new(
                    state.tiling.preview.id.clone(),
                    local.to_physical_precise_round(scale),
                    state.tiling.preview.commit,
                    crate::tiling::PREVIEW_COLOR,
                    Kind::Unspecified,
                )));
            }
            if let Ok(space) = smithay::desktop::space::space_render_elements(renderer, [&state.space], output, 1.0) {
                elements.extend(space.into_iter().map(OutputElement::Space));
            }
        }
    }
    (elements, CLEAR)
}

/// Draws one frame of `output` into `framebuffer` (nested backend,
/// screenshots). Returns the damaged regions, or None if nothing changed
/// since the tracker's last frame.
pub fn compose(
    renderer: &mut GlesRenderer,
    framebuffer: &mut GlesTarget<'_>,
    output: &Output,
    state: &WhiteRose,
    tracker: &mut OutputDamageTracker,
    age: usize,
) -> Result<Option<Vec<Rectangle<i32, Physical>>>, String> {
    let (elements, clear) = output_elements(renderer, output, state, Vec::new());
    tracker
        .render_output(renderer, framebuffer, age, &elements, clear)
        .map(|r| r.damage.cloned())
        .map_err(|e| format!("{e:?}"))
}

/// Tells `surface` (and its subsurfaces) the scale and rotation of the
/// output it's on, so it draws buffers that need no scaling. Smithay only
/// sends what changed.
pub fn send_scale(surface: &smithay::reexports::wayland_server::protocol::wl_surface::WlSurface, output: &Output) {
    let scale = output.current_scale();
    let transform = output.current_transform();
    smithay::wayland::compositor::with_surface_tree_downward(
        surface,
        (),
        |_, _, _| smithay::wayland::compositor::TraversalAction::DoChildren(()),
        |s, states, _| {
            smithay::wayland::compositor::send_surface_state(s, states, scale.integer_scale(), transform);
            smithay::wayland::fractional_scale::with_fractional_scale(states, |f| {
                f.set_preferred_scale(scale.fractional_scale())
            });
        },
        |_, _, _| true,
    );
    // Its menus and other popups too.
    for (popup, _) in smithay::desktop::PopupManager::popups_for_surface(surface) {
        send_scale(popup.wl_surface(), output);
    }
}

/// Tells clients that drew this frame they may draw the next one. Only
/// visible surfaces get a callback, so hidden ones (the bar under a fullscreen
/// video, windows behind it) stop drawing entirely.
pub fn send_frame_callbacks(output: &Output, state: &mut WhiteRose) {
    let fullscreen = state.fullscreen_on(output).cloned();
    let elapsed = state.start_time.elapsed();
    // Locked: only the lock screen draws; everything else waits, unseen.
    if state.is_locked() {
        for lock in state.lock.surfaces() {
            send_scale(lock.wl_surface(), output);
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
            if let Some(t) = window.toplevel() {
                send_scale(t.wl_surface(), output);
            }
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
                send_scale(layer.wl_surface(), output);
                layer.send_frame(output, elapsed, Some(Duration::ZERO), |_, _| Some(output.clone()));
            }
        }
        layers.cleanup();
    }
    state.space.refresh();
    state.popups.cleanup();
    let _ = state.display_handle.flush_clients();
}
