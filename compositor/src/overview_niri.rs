//! The overview, niri-style (`overview.style` niri; Figma "Overview niri -
//! SUPER + W"): under the bar the desktop zooms out to half size and the
//! workspaces sit side by side, live, as they're laid out (tiles, floating
//! windows, the wallpaper), the current one in the middle. Scrolling, the
//! arrow keys or three fingers left or right move along them; a click on
//! a window goes to it, on a workspace's empty space goes there, and a
//! window dragged onto another workspace moves there.
//!
//! It shares the rest with the GNOME style (overview.rs): opening and
//! closing, the pointer, the drag, the scaled drawing. Only where things
//! are differs: every workspace's windows go through one `Zoom`, so the
//! strip slides with the workspace slide (workspace_slide.rs) and opening
//! zooms out of exactly what was on screen.

use std::time::Instant;

use smithay::{
    backend::renderer::gles::GlesRenderer,
    desktop::{layer_map_for_output, Window},
    output::Output,
    utils::{Point, Rectangle},
    wayland::shell::wlr_layer::Layer as WlrLayer,
};

use crate::{
    overview::{self, Rect, Scene, Shapes},
    render::OutputElement,
    state::WhiteRose,
};

/// How small the workspaces get.
pub const ZOOM: f64 = 0.5;
/// Between workspaces, zoomed out.
pub const STRIP_GAP: f64 = 32.0;
/// A touchpad scroll this far moves one workspace (a wheel: each notch).
const SCROLL_STEP: f64 = 80.0;
/// The window title (the shell's) sits this far under its workspace.
const TITLE_GAP: f64 = 16.0;

/// Where everything goes on an output `w` × `h` whose usable area starts
/// at `top`, with the strip at workspace `pos` (fractional while it
/// slides).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Zoom {
    pub w: f64,
    pub h: f64,
    pub top: f64,
    pub pos: f64,
}

impl Zoom {
    /// Where `r` (on workspace `ws`, output-local as on the desktop) is
    /// drawn `t` of the way zoomed out (0: the desktop as it is, 1: open).
    /// Straight lines in `t`, so opening can go from one to the other.
    pub fn at(&self, ws: usize, r: Rect, t: f64) -> Rect {
        let z = 1.0 + (ZOOM - 1.0) * t;
        let dx = (ws as f64 - self.pos) * (self.w * z + STRIP_GAP * t);
        // The middle of the area under the bar, once open.
        let cy = self.h / 2.0 + self.top / 2.0 * t;
        Rect::new(self.w / 2.0 + (r.x - self.w / 2.0) * z + dx, cy + (r.y - self.h / 2.0) * z, r.w * z, r.h * z)
    }

    /// Workspace `ws` itself, `t` of the way zoomed out.
    pub fn cell(&self, ws: usize, t: f64) -> Rect {
        self.at(ws, Rect::new(0.0, 0.0, self.w, self.h), t)
    }
}

impl WhiteRose {
    /// Where the strip is: the current workspace, or between two while it
    /// slides.
    pub fn strip_at(&self, now: Instant) -> f64 {
        match self.workspace_sliding(now) {
            Some(s) => s.from as f64 + (s.to as f64 - s.from as f64) * s.along.get(now).clamp(0.0, 1.0),
            None => self.workspaces.active() as f64,
        }
    }

    /// The niri-style overview on `output`: every workspace's windows on
    /// it, where they are on the desktop and where they're drawn zoomed
    /// out, and each workspace's place.
    pub(crate) fn niri_scene(&self, output: &Output) -> Option<Scene> {
        let geo = self.space.output_geometry(output)?;
        let zoom = Zoom {
            w: geo.size.w as f64,
            h: geo.size.h as f64,
            top: (self.usable_area_on(output).loc.y - geo.loc.y) as f64,
            pos: self.strip_at(Instant::now()),
        };
        let mut windows = Vec::new();
        for ws in 0..self.workspaces.count() {
            for win in self.workspaces.windows(ws) {
                if self.minimized.contains(win) || Some(win) == self.overview.dragging.as_ref() {
                    continue;
                }
                let Some(g) = self.desktop_geometry(win) else { continue };
                let mid = g.loc + Point::from((g.size.w / 2, g.size.h / 2));
                if !geo.contains(mid) || g.size.w <= 0 || g.size.h <= 0 {
                    continue;
                }
                let r = Rect::new(
                    (g.loc.x - geo.loc.x) as f64,
                    (g.loc.y - geo.loc.y) as f64,
                    g.size.w as f64,
                    g.size.h as f64,
                );
                windows.push((win.clone(), zoom.at(ws, r, 0.0), zoom.at(ws, r, 1.0)));
            }
        }
        let cells = (0..self.workspaces.count()).map(|i| zoom.cell(i, 1.0)).collect();
        Some(Scene { output: output.clone(), origin: geo.loc, windows, cells, zoom: Some(zoom) })
    }

    /// Where `w` is on its workspace (global): where it's mapped, or where
    /// it was when its workspace was hidden.
    fn desktop_geometry(&self, w: &Window) -> Option<Rectangle<i32, smithay::utils::Logical>> {
        if let Some(g) = self.space.element_geometry(w) {
            return Some(g);
        }
        let at = self.hidden_at.iter().find(|(h, _)| h == w).map(|(_, l)| *l)?;
        let g = w.geometry();
        Some(Rectangle::new(at + g.loc, g.size))
    }

    /// Scrolled `amount` (a wheel notch is 15; `finger`: a touchpad) with
    /// the niri-style overview open: on to the next workspace or back.
    /// False when it isn't that overview's.
    pub fn overview_scroll(&mut self, amount: f64, finger: bool) -> bool {
        if self.overview_style != "niri" {
            return false;
        }
        let step = if finger { SCROLL_STEP } else { 15.0 };
        self.overview.scroll += amount;
        if self.overview.scroll.abs() >= step {
            let dir = self.overview.scroll.signum() as i32;
            self.overview.scroll = 0.0;
            self.overview_step(dir);
        }
        true
    }

    /// The fingers stopped scrolling: start counting again.
    pub fn overview_scroll_stop(&mut self) {
        self.overview.scroll = 0.0;
    }

    /// The workspace `dir` along from this one (an arrow key, a scroll),
    /// None past either end.
    pub fn overview_step_target(&self, dir: i32) -> Option<usize> {
        let to = self.workspaces.active() as i64 + dir as i64;
        (0..self.workspaces.count() as i64).contains(&to).then_some(to as usize)
    }

    fn overview_step(&mut self, dir: i32) {
        if let Some(ws) = self.overview_step_target(dir) {
            self.switch_workspace(ws);
            self.overview.hovered = None;
            self.overview_changed();
        }
    }

    /// The window title's place for a window zoomed out to `r`: under its
    /// workspace.
    pub(crate) fn niri_title_y(scene: &Scene, r: &Rect) -> Option<i32> {
        let mid = (r.x + r.w / 2.0, r.y + r.h / 2.0);
        let cell = scene.cells.iter().find(|c| c.contains(mid))?;
        Some((cell.y + cell.h + TITLE_GAP).round() as i32)
    }
}

/// What `output` shows with the niri-style overview open (front to back).
pub(crate) fn elements(
    renderer: &mut GlesRenderer,
    output: &Output,
    state: &WhiteRose,
    scene: &Scene,
) -> Vec<OutputElement> {
    let Some(zoom) = scene.zoom else { return Vec::new() };
    let scale = output.current_scale().fractional_scale();
    let t = state.overview_shown().clamp(0.0, 1.0);
    let ov = &state.overview;
    let mut shapes = Shapes::new(ov, scale);
    let mut out = Vec::new();
    let screen = Rect::new(0.0, 0.0, zoom.w, zoom.h);

    // The bar and the shell's layers (the window's title) over everything.
    overview::layers(renderer, output, scale, &[WlrLayer::Overlay, WlrLayer::Top], &mut out);

    // The window being dragged, as big as it is zoomed out, under the
    // pointer where it was picked up.
    let local = |p: Point<f64, smithay::utils::Logical>| (p.x - scene.origin.x as f64, p.y - scene.origin.y as f64);
    let pointer = local(ov.pointer);
    let target = ov.dragging.as_ref().and_then(|_| scene.cells.iter().position(|c| c.contains(pointer)));
    if let Some(w) = ov.dragging.as_ref() {
        let g = w.geometry();
        if g.size.w > 0 && state.output_at(ov.pointer).as_ref() == Some(output) {
            let (dw, dh) = (g.size.w as f64 * ZOOM, g.size.h as f64 * ZOOM);
            let r = Rect::new(pointer.0 - ov.grab.0 * dw, pointer.1 - ov.grab.1 * dh, dw, dh);
            overview::scaled_window(renderer, w, r, scale, 0.9, &mut out);
        }
    }

    // Outlines: the window under the pointer, the current workspace, the
    // workspace a window is dragged over.
    if t >= 1.0 {
        if let Some((_, _, r)) = scene.windows.iter().find(|(w, _, _)| Some(w) == ov.hovered.as_ref()) {
            shapes.ring(&mut out, *r, overview::OUTLINE, 1.0);
        }
    }
    shapes.ring(&mut out, zoom.cell(state.workspaces.active(), t), overview::OUTLINE, t);
    if let Some(i) = target.filter(|i| *i != state.workspaces.active()) {
        shapes.ring(&mut out, zoom.cell(i, t), overview::TARGET, 1.0);
    }

    // Every workspace's windows (the strip's workspaces never overlap).
    for (w, now, to) in scene.windows.iter().rev() {
        let r = now.lerp(to, t);
        if r.overlaps(&screen) {
            overview::scaled_window(renderer, w, r, scale, state.window_alpha(w), &mut out);
        }
    }

    // Each workspace on screen: its wallpaper (and what sits on it),
    // scaled, over a plain one for when there's none.
    let map = layer_map_for_output(output);
    for i in 0..state.workspaces.count() {
        let c = zoom.cell(i, t);
        if !c.overlaps(&screen) {
            continue;
        }
        let z = c.w / zoom.w.max(1.0);
        for layer in [WlrLayer::Bottom, WlrLayer::Background] {
            for l in map.layers_on(layer).rev() {
                let loc = map.layer_geometry(l).map(|g| g.loc).unwrap_or_default();
                let at = Point::<f64, smithay::utils::Logical>::from((c.x + loc.x as f64 * z, c.y + loc.y as f64 * z));
                overview::layer_scaled(renderer, l, at, scale, z, &mut out);
            }
        }
        shapes.solid(&mut out, c, overview::CELL, 1.0);
    }

    // Behind it all: the wallpaper through glass.
    shapes.solid(&mut out, screen, overview::BACKDROP, overview::GLASS);
    overview::layers(renderer, output, scale, &[WlrLayer::Bottom, WlrLayer::Background], &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const Z: Zoom = Zoom { w: 1440.0, h: 808.0, top: 26.0, pos: 2.0 };

    #[test]
    fn closed_it_is_the_desktop_as_it_is() {
        let r = Rect::new(12.0, 38.0, 700.0, 500.0);
        assert_eq!(Z.at(2, r, 0.0), r);
        assert_eq!(Z.cell(3, 0.0), Rect::new(1440.0, 0.0, 1440.0, 808.0), "the next just off the right edge");
        assert_eq!(Z.cell(1, 0.0).x, -1440.0);
    }

    #[test]
    fn open_the_current_one_is_half_size_in_the_middle_under_the_bar() {
        let c = Z.cell(2, 1.0);
        assert_eq!((c.w, c.h), (720.0, 404.0));
        assert_eq!(c.x, 360.0);
        assert_eq!(c.y, 215.0, "centred in the area under the bar");
        let next = Z.cell(3, 1.0);
        assert_eq!(next.x, c.x + c.w + STRIP_GAP);
        assert!(next.x < 1440.0, "its neighbours show at the sides");
        // A window keeps its place on its workspace.
        let r = Z.at(2, Rect::new(720.0, 404.0, 100.0, 100.0), 1.0);
        assert_eq!((r.x, r.y, r.w), (c.x + 360.0, c.y + 202.0, 50.0));
    }

    #[test]
    fn opening_goes_in_a_straight_line() {
        let r = Rect::new(100.0, 60.0, 400.0, 300.0);
        for ws in [1, 2, 4] {
            let mid = Z.at(ws, r, 0.5);
            let lerp = Z.at(ws, r, 0.0).lerp(&Z.at(ws, r, 1.0), 0.5);
            assert!((mid.x - lerp.x).abs() < 1e-9 && (mid.y - lerp.y).abs() < 1e-9 && (mid.w - lerp.w).abs() < 1e-9);
        }
    }

    #[test]
    fn the_strip_slides_between_workspaces() {
        let half = Zoom { pos: 2.5, ..Z };
        let (a, b) = (half.cell(2, 1.0), half.cell(3, 1.0));
        assert!((a.x + a.w + STRIP_GAP / 2.0 - 720.0).abs() < 1e-9, "half way: the gap between them in the middle");
        assert_eq!(b.x, a.x + a.w + STRIP_GAP);
    }
}
