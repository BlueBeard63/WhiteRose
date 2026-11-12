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
            utils::CropRenderElement,
            AsRenderElements, Kind,
        },
        gles::{GlesRenderer, GlesTarget},
    },
    desktop::{layer_map_for_output, LayerSurface, PopupManager, Window},
    output::Output,
    utils::{Logical, Physical, Point, Rectangle, Scale},
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
    /// Windows and layer surfaces.
    Surface=WaylandSurfaceRenderElement<GlesRenderer>,
    /// A tiled window cut to its tile.
    Cropped=CropRenderElement<WaylandSurfaceRenderElement<GlesRenderer>>,
    /// Pixels the compositor draws: the pointer, tab strips.
    Image=MemoryRenderBufferRenderElement<GlesRenderer>,
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
            // Scratchpads shown here (or sliding out) go over it, top first.
            let out = state.space.output_geometry(output).unwrap_or_default();
            let pads: Vec<&Window> = state.scratchpads.drawn_on(output).collect();
            for pad in pads.into_iter().rev() {
                let Some(at) = state.space.element_location(pad) else { continue };
                let loc = at - pad.geometry().loc - out.loc;
                if !sliding_pad(renderer, output, out, state, pad, loc, &mut elements) {
                    elements.extend(
                        pad.render_elements::<WaylandSurfaceRenderElement<_>>(
                            renderer,
                            loc.to_physical_precise_round(scale),
                            Scale::from(scale),
                            state.window_alpha(pad),
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
                        state.window_alpha(window),
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
            // A zone grid (SHIFT while dragging), under the zone it'll fill.
            for (r, id) in &state.snap.grid {
                if let Some(r) = r.intersection(out) {
                    let local = Rectangle::new(r.loc - out.loc, r.size);
                    elements.push(OutputElement::Solid(SolidColorRenderElement::new(
                        id.clone(),
                        local.to_physical_precise_round(scale),
                        state.snap.grid_commit,
                        crate::snapping::GRID_COLOR,
                        Kind::Unspecified,
                    )));
                }
            }
            space_elements(renderer, output, state, &mut elements);
        }
    }
    // A scratchpad sliding: the next frame too, until it ends.
    if state.scratchpads.animating() {
        state.schedule_redraw();
    }
    (elements, CLEAR)
}

/// The space as `output` shows it, front to back: the top and overlay
/// layers, the windows, the tab strips (behind every window, so a dialog
/// over one covers it), then the bottom and background layers. As Smithay's
/// `space_render_elements`, except that a tiled window is cut to its tile
/// (`clip_of`), so an app that draws bigger never covers its neighbours.
fn space_elements(renderer: &mut GlesRenderer, output: &Output, state: &WhiteRose, elements: &mut Vec<OutputElement>) {
    let Some(out) = state.space.output_geometry(output) else { return };
    let scale = output.current_scale().fractional_scale();
    // Front to back: by layer (overlay over top, bottom over background),
    // then the newest first within one. (The map keeps them in the order
    // they opened, whatever their layer: a top surface opened after the bar
    // moved to overlay would otherwise cover it.)
    let mut layers: Vec<(LayerSurface, Point<i32, Logical>)> = {
        let map = layer_map_for_output(output);
        map.layers().rev().filter_map(|l| map.layer_geometry(l).map(|g| (l.clone(), g.loc))).collect()
    };
    layers.sort_by_key(|(l, _)| std::cmp::Reverse(l.layer() as u32));
    let lower = |l: &LayerSurface| matches!(l.layer(), WlrLayer::Background | WlrLayer::Bottom);
    let layer_elements = |renderer: &mut GlesRenderer, below: bool, elements: &mut Vec<OutputElement>| {
        for (layer, loc) in layers.iter().filter(|(l, _)| lower(l) == below) {
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
    };
    layer_elements(renderer, false, elements);
    for w in state.space.elements().rev() {
        let Some(bbox) = state.space.element_bbox(w) else { continue };
        if !bbox.overlaps(out) || !state.shows_on(w, output) {
            continue;
        }
        let Some(at) = state.space.element_location(w) else { continue };
        let loc = at - w.geometry().loc - out.loc;
        if sliding_pad(renderer, output, out, state, w, loc, elements) {
            continue;
        }
        // (A rule's opacity, focused or not.)
        let alpha = state.window_alpha(w);
        match state.clip_of(w) {
            None => elements.extend(
                w.render_elements::<WaylandSurfaceRenderElement<_>>(
                    renderer,
                    loc.to_physical_precise_round(scale),
                    Scale::from(scale),
                    alpha,
                )
                .into_iter()
                .map(OutputElement::Surface),
            ),
            Some(clip) => {
                let crop = Rectangle::new(clip.loc - out.loc, clip.size).to_physical_precise_round(scale);
                clipped_window(renderer, w, loc.to_physical_precise_round(scale), scale, alpha, crop, elements);
            }
        }
    }
    elements.extend(crate::tab_strip::elements(renderer, output, state));
    layer_elements(renderer, true, elements);
}

/// A scratchpad sliding from the island: drawn `shown` of the way down
/// from above the usable area's top, and cut off there, so it comes out
/// from under the bar. `loc` is where it's drawn when it's still (in the
/// output's logical space, `out` the output's geometry). False if `w`
/// isn't sliding.
fn sliding_pad(
    renderer: &mut GlesRenderer,
    output: &Output,
    out: Rectangle<i32, Logical>,
    state: &WhiteRose,
    w: &Window,
    loc: Point<i32, Logical>,
    elements: &mut Vec<OutputElement>,
) -> bool {
    let Some(slide) = state.scratchpads.slide_of(w) else { return false };
    let Some(geo) = state.space.element_geometry(w) else { return false };
    let scale = output.current_scale().fractional_scale();
    let shown = slide.shown(std::time::Instant::now(), state.scratchpads.slide_length);
    let top = state.usable_area_on(output).loc.y;
    let dy = crate::scratchpad::slide_offset(shown, geo.loc.y + geo.size.h, top);
    let below = (top - out.loc.y).clamp(0, out.size.h);
    let crop = Rectangle::<i32, Logical>::new((0, below).into(), (out.size.w, out.size.h - below).into())
        .to_physical_precise_round(scale);
    let at = Point::<i32, Logical>::from((loc.x, loc.y + dy)).to_physical_precise_round(scale);
    clipped_window(renderer, w, at, scale, state.window_alpha(w), crop, elements);
    true
}

/// A tiled window's elements, front to back: its popups whole (a menu may
/// reach past the tile), then its own surfaces cut to `crop` (the tile, in
/// the output's physical pixels), at `alpha`. As `Window::render_elements`
/// otherwise.
fn clipped_window(
    renderer: &mut GlesRenderer,
    w: &Window,
    location: Point<i32, Physical>,
    scale: f64,
    alpha: f32,
    crop: Rectangle<i32, Physical>,
    elements: &mut Vec<OutputElement>,
) {
    let Some(toplevel) = w.toplevel() else { return };
    let surface = toplevel.wl_surface();
    for (popup, offset) in PopupManager::popups_for_surface(surface) {
        let at = (w.geometry().loc + offset - popup.geometry().loc).to_physical_precise_round(scale);
        elements.extend(
            render_elements_from_surface_tree::<_, WaylandSurfaceRenderElement<_>>(
                renderer,
                popup.wl_surface(),
                location + at,
                Scale::from(scale),
                alpha,
                Kind::Unspecified,
            )
            .into_iter()
            .map(OutputElement::Surface),
        );
    }
    elements.extend(
        render_elements_from_surface_tree::<_, WaylandSurfaceRenderElement<_>>(
            renderer,
            surface,
            location,
            Scale::from(scale),
            alpha,
            Kind::Unspecified,
        )
        .into_iter()
        .filter_map(|e| CropRenderElement::from_element(e, scale, crop))
        .map(OutputElement::Cropped),
    );
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
    // Windows being previewed keep drawing even where they can't be seen.
    let exporting = state.exporting_windows();
    for window in state.space.elements() {
        // A tiled window gets frames (and its scale) only from the outputs
        // its tile is on, even if the app draws past it.
        if !state.shows_on(window, output) && !exporting.contains(window) {
            continue;
        }
        if fullscreen.as_ref().is_none_or(|f| f == window) {
            if let Some(t) = window.toplevel() {
                send_scale(t.wl_surface(), output);
            }
            window.send_frame(output, elapsed, Some(Duration::ZERO), |_, _| Some(output.clone()));
        }
    }
    // Those hidden on another workspace too.
    for window in exporting {
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
