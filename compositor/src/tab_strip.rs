//! The tab strip above a grouped tile, as a small island (Figma: "Grouping
//! — tab strip options", C): a thin black band across the top of the tile
//! with the tabs hanging from it in the middle on concave ears, as the
//! island hangs from the bar. Each tab is its title; the shown one sits on
//! a lighter pill. Beside the island the strip is see-through.
//!
//! The compositor draws it itself: each strip is drawn once into a pixel
//! buffer at the output's real resolution (sharp at 125%, 150%, ...) and
//! kept until its size, titles or shown tab change, so an idle desktop
//! draws nothing new. `layout` places the tabs for both drawing and clicks.

use std::{cell::RefCell, collections::HashMap, sync::OnceLock};

use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            element::{
                memory::{MemoryRenderBuffer, MemoryRenderBufferRenderElement},
                Kind,
            },
            gles::GlesRenderer,
        },
    },
    desktop::Window,
    output::Output,
    utils::{Logical, Point, Rectangle, Transform},
};

use crate::{render::OutputElement, state::WhiteRose};

/// The strip of one grouped tile on screen.
#[derive(Debug, Clone)]
pub struct Strip {
    pub output: Output,
    /// Global coordinates: the top of the tile.
    pub rect: Rectangle<i32, Logical>,
    pub tabs: Vec<Window>,
    pub shown: usize,
}

impl Strip {
    /// The middle of tab `i` (global coordinates).
    pub fn tab_middle(&self, i: usize) -> Option<Point<f64, Logical>> {
        let (x0, x1) = *layout(self.rect.size.w as f64, &tabs_of(&self.tabs, ICON as u32)).tabs.get(i)?;
        Some((self.rect.loc.x as f64 + (x0 + x1) / 2.0, self.rect.loc.y as f64 + BAND + ISLAND / 2.0).into())
    }

    /// The tab under `pos`, if it's on this strip's island.
    pub fn tab_at(&self, pos: Point<f64, Logical>) -> Option<&Window> {
        if !self.rect.to_f64().contains(pos) || self.tabs.is_empty() {
            return None;
        }
        let l = layout(self.rect.size.w as f64, &tabs_of(&self.tabs, ICON as u32));
        let (x, y) = (pos.x - self.rect.loc.x as f64, pos.y - self.rect.loc.y as f64);
        if !(BAND..BAND + ISLAND).contains(&y) {
            return None;
        }
        l.tabs.iter().position(|(x0, x1)| (*x0..*x1).contains(&x)).and_then(|i| self.tabs.get(i))
    }
}

// Colours as (r, g, b): the island's black, the shown tab's pill, text.
const PANEL: [u8; 3] = [0x0F, 0x0F, 0x0F];
const SHOWN: [u8; 3] = [0x2A, 0x2A, 0x2E];
const TEXT: [u8; 3] = [0xF4, 0xF4, 0xF5];
const TEXT_DIM: [u8; 3] = [0x8E, 0x8E, 0x96];
/// Sizes in logical pixels: the band, the island under it, the ears, the
/// island's bottom corners; its padding, a tab's (each side), the gap
/// between tabs, a tab pill's height and corners; the longest a tab gets.
pub const BAND: f64 = 6.0;
pub const ISLAND: f64 = 30.0;
const EAR: f64 = 12.0;
const CORNER: f64 = 14.0;
const ISLAND_PAD: f64 = 6.0;
const TAB_PAD: f64 = 12.0;
const TAB_GAP: f64 = 2.0;
const PILL: f64 = 24.0;
const PILL_RADIUS: f64 = 9.0;
const TAB_MAX: f64 = 220.0;
/// An app's icon before its title, and the space after it.
const ICON: f64 = 16.0;
const ICON_GAP: f64 = 7.0;
/// How strongly a tab that isn't shown draws its icon.
const ICON_DIM: u32 = 170;
const FONT_SIZE: f64 = 12.5;

/// Where things go on a strip `w` logical pixels wide: the island's left
/// edge and width, and each tab's [x0, x1).
#[derive(Debug, Clone, PartialEq)]
pub struct StripLayout {
    pub island_x: f64,
    pub island_w: f64,
    pub tabs: Vec<(f64, f64)>,
}

/// One tab: its title, and its app's icon if it has one (app_icons.rs).
#[derive(Debug, Clone, Default)]
pub struct Tab {
    pub title: String,
    pub icon: Option<std::sync::Arc<crate::app_icons::Icon>>,
}

/// The tabs for `windows`, icons `px` pixels big.
fn tabs_of(windows: &[Window], px: u32) -> Vec<Tab> {
    windows
        .iter()
        .map(|w| {
            let (app, _) = crate::ipc::toplevel_ids(w);
            Tab { title: label(w), icon: app.and_then(|a| crate::app_icons::icon(&a, px)) }
        })
        .collect()
}

/// Tabs as wide as their icon and title (up to TAB_MAX); if the island
/// would be wider than the tile allows, they share the room evenly.
pub fn layout(w: f64, tabs: &[Tab]) -> StripLayout {
    let n = tabs.len().max(1) as f64;
    let room = (w - 2.0 * (EAR + 8.0) - 2.0 * ISLAND_PAD - (n - 1.0) * TAB_GAP).max(n * 2.0 * TAB_PAD);
    // (+2: a title exactly its tab's room would lose a letter to rounding.)
    let mut widths: Vec<f64> = tabs
        .iter()
        .map(|t| {
            let icon = if t.icon.is_some() { ICON + ICON_GAP } else { 0.0 };
            (icon + text_width(&t.title, FONT_SIZE as f32).ceil() as f64 + 2.0 + 2.0 * TAB_PAD).min(TAB_MAX)
        })
        .collect();
    if widths.iter().sum::<f64>() > room {
        widths = vec![(room / n).floor(); tabs.len()];
    }
    let island_w = widths.iter().sum::<f64>() + (n - 1.0) * TAB_GAP + 2.0 * ISLAND_PAD;
    let island_x = ((w - island_w) / 2.0).round();
    let mut x = island_x + ISLAND_PAD;
    let tabs = widths
        .iter()
        .map(|tw| {
            let t = (x, x + tw);
            x += tw + TAB_GAP;
            t
        })
        .collect();
    StripLayout { island_x, island_w, tabs }
}

fn text_width(s: &str, size: f32) -> f32 {
    s.chars().map(|c| font().metrics(c, size).advance_width).sum()
}

fn font() -> &'static fontdue::Font {
    static FONT: OnceLock<fontdue::Font> = OnceLock::new();
    FONT.get_or_init(|| {
        fontdue::Font::from_bytes(&include_bytes!("../../shell/fonts/Geist-Medium.ttf")[..], Default::default())
            .expect("the bundled Geist font loads")
    })
}

/// True if (x, y) (logical) is inside the rounded rectangle.
fn in_round_rect(x: f64, y: f64, (x0, y0, x1, y1): (f64, f64, f64, f64), top: f64, bottom: f64) -> bool {
    if x < x0 || x >= x1 || y < y0 || y >= y1 {
        return false;
    }
    let corner = |cx: f64, cy: f64, r: f64| (x - cx).powi(2) + (y - cy).powi(2) <= r * r;
    let r = if y < y0 + top { top } else { bottom };
    if r <= 0.0 || (y >= y0 + top && y < y1 - bottom) {
        return true;
    }
    let cy = if y < y0 + top { y0 + top } else { y1 - bottom };
    if x < x0 + r {
        corner(x0 + r, cy, r)
    } else if x >= x1 - r {
        corner(x1 - r, cy, r)
    } else {
        true
    }
}

/// True if (x, y) (logical) is on the island: the band, the body under
/// it with round bottom corners, and the ears joining them.
fn in_island(x: f64, y: f64, w: f64, l: &StripLayout) -> bool {
    if (0.0..BAND).contains(&y) && (0.0..w).contains(&x) {
        return true;
    }
    let (bx0, bx1) = (l.island_x, l.island_x + l.island_w);
    if in_round_rect(x, y, (bx0, BAND, bx1, BAND + ISLAND), 0.0, CORNER) {
        return true;
    }
    // The ears: under the band, outside a circle in the corner they fill.
    let ear = |cx: f64| (BAND..BAND + EAR).contains(&y) && (x - cx).powi(2) + (y - (BAND + EAR)).powi(2) >= EAR * EAR;
    ((bx0 - EAR)..bx0).contains(&x) && ear(bx0 - EAR) || (bx1..(bx1 + EAR)).contains(&x) && ear(bx1 + EAR)
}

/// How much of pixel (px, py) `inside` covers, 0–255 (4x4 samples).
fn coverage(px: usize, py: usize, scale: f64, inside: impl Fn(f64, f64) -> bool) -> u32 {
    let mut n = 0;
    for sy in 0..4 {
        for sx in 0..4 {
            let (x, y) = ((px as f64 + (sx as f64 + 0.5) / 4.0) / scale, (py as f64 + (sy as f64 + 0.5) / 4.0) / scale);
            n += inside(x, y) as u32;
        }
    }
    n * 255 / 16
}

/// Paints `colour` over pixel i at coverage `a` (premultiplied BGRA).
fn paint(px: &mut [u8], i: usize, [r, g, b]: [u8; 3], a: u32) {
    for (k, c) in [b, g, r, 0xFF].into_iter().enumerate() {
        px[i + k] = ((c as u32 * a + px[i + k] as u32 * (255 - a)) / 255) as u8;
    }
}

/// Draws a strip `w` x `h` pixels (Argb8888, premultiplied: B, G, R, A in
/// memory) at `scale` pixels per logical pixel.
pub fn draw(w: i32, h: i32, scale: f64, tabs: &[Tab], shown: usize) -> Vec<u8> {
    let (w, h) = (w.max(1) as usize, h.max(1) as usize);
    let mut px = vec![0u8; w * h * 4];
    let lw = w as f64 / scale;
    let l = layout(lw, tabs);
    let pill_top = BAND + (ISLAND - PILL) / 2.0 - 1.0;
    let pill = l.tabs.get(shown).map(|(x0, x1)| (*x0, pill_top, *x1, pill_top + PILL));
    // Only the rows the island covers need looking at.
    let rows = (((BAND + ISLAND) * scale).ceil() as usize + 1).min(h);
    for y in 0..rows {
        for x in 0..w {
            let i = (y * w + x) * 4;
            let a = coverage(x, y, scale, |lx, ly| in_island(lx, ly, lw, &l));
            if a > 0 {
                paint(&mut px, i, PANEL, a);
            }
            if let Some(p) = pill {
                let a = coverage(x, y, scale, |lx, ly| in_round_rect(lx, ly, p, PILL_RADIUS, PILL_RADIUS));
                if a > 0 {
                    paint(&mut px, i, SHOWN, a);
                }
            }
        }
    }
    let size = (FONT_SIZE * scale) as f32;
    let (ty0, ty1) = ((pill_top * scale).round() as usize, ((pill_top + PILL) * scale).round() as usize);
    for (i, (tab, (x0, x1))) in tabs.iter().zip(&l.tabs).enumerate() {
        let colour = if i == shown { TEXT } else { TEXT_DIM };
        let mut start = x0 + TAB_PAD;
        if let Some(icon) = &tab.icon {
            let (ix, iy) =
                ((start * scale).round() as usize, ((pill_top + (PILL - ICON) / 2.0) * scale).round() as usize);
            blit(&mut px, w, h, ix, iy, icon, if i == shown { 255 } else { ICON_DIM });
            start += ICON + ICON_GAP;
        }
        let (tx0, tx1) = ((start * scale).round() as usize, ((x1 - TAB_PAD) * scale).round() as usize);
        text(&mut px, w, (ty0, ty1), tx0, tx1, &tab.title, size, colour);
    }
    px
}

/// Draws `icon` (premultiplied) with its top left at (x, y), at `alpha`.
fn blit(px: &mut [u8], w: usize, h: usize, x: usize, y: usize, icon: &crate::app_icons::Icon, alpha: u32) {
    let n = icon.size as usize;
    for iy in 0..n.min(h.saturating_sub(y)) {
        for ix in 0..n.min(w.saturating_sub(x)) {
            let s = (iy * n + ix) * 4;
            let d = ((y + iy) * w + x + ix) * 4;
            let a = icon.bgra[s + 3] as u32 * alpha / 255;
            for k in 0..4 {
                let src = icon.bgra[s + k] as u32 * alpha / 255;
                px[d + k] = (src + px[d + k] as u32 * (255 - a) / 255).min(255) as u8;
            }
        }
    }
}

/// Writes `s` between `x0` and `x1`, centred vertically; cut short with
/// an ellipsis if it doesn't fit.
#[allow(clippy::too_many_arguments)]
fn text(
    px: &mut [u8],
    w: usize,
    (top, bottom): (usize, usize),
    x0: usize,
    x1: usize,
    s: &str,
    size: f32,
    colour: [u8; 3],
) {
    let h = bottom.saturating_sub(top);
    let font = font();
    let room = x1.saturating_sub(x0) as f32;
    let advance = |c: char| font.metrics(c, size).advance_width;
    let mut chars: Vec<char> = s.chars().collect();
    if chars.iter().map(|c| advance(*c)).sum::<f32>() > room {
        let ellipsis = advance('…');
        let mut used = 0.0;
        let fits = chars
            .iter()
            .take_while(|c| {
                used += advance(**c);
                used + ellipsis <= room
            })
            .count();
        chars.truncate(fits);
        // No dangling space before the ellipsis.
        while chars.last().is_some_and(|c| c.is_whitespace()) {
            chars.pop();
        }
        chars.push('…');
    }
    let Some(line) = font.horizontal_line_metrics(size) else { return };
    let baseline = top as i32 + ((h as f32 - (line.ascent - line.descent)) / 2.0 + line.ascent).round() as i32;
    let mut pen = x0 as f32;
    for c in chars {
        let (m, bitmap) = font.rasterize(c, size);
        let gx = (pen + m.xmin as f32).round() as i32;
        let gy = baseline - m.height as i32 - m.ymin;
        for row in 0..m.height {
            for col in 0..m.width {
                let (x, y) = (gx + col as i32, gy + row as i32);
                if x < x0 as i32 || x >= x1.max(x0) as i32 || y < top as i32 || y >= bottom as i32 {
                    continue;
                }
                let a = bitmap[row * m.width + col] as u32;
                paint(px, (y as usize * w + x as usize) * 4, colour, a);
            }
        }
        pen += m.advance_width;
    }
}

/// What a drawn strip shows: its size in pixels, the titles, the shown tab.
type StripKey = (i32, i32, Vec<(String, Option<String>)>, usize);

/// Drawn strips, by what they show.
#[derive(Debug, Default)]
pub struct StripImages {
    images: RefCell<HashMap<StripKey, MemoryRenderBuffer>>,
}

/// A window's tab label: its title, else its app id.
fn label(w: &Window) -> String {
    let (app, title) = crate::ipc::toplevel_ids(w);
    title.filter(|t| !t.is_empty()).or(app).unwrap_or_else(|| "Window".into())
}

/// The tab strips on `output`, ready to draw (behind the windows).
pub fn elements(renderer: &mut GlesRenderer, output: &Output, state: &WhiteRose) -> Vec<OutputElement> {
    let scale = output.current_scale().fractional_scale();
    let Some(origin) = state.space.output_geometry(output).map(|g| g.loc) else { return Vec::new() };
    let mut images = state.tiling.strip_images.images.borrow_mut();
    // Titles change; old pictures go now and then.
    if images.len() > 64 {
        images.clear();
    }
    let mut out = Vec::new();
    for s in state.tiling.strips.iter().filter(|s| s.output == *output) {
        let (w, h) = ((s.rect.size.w as f64 * scale).round() as i32, (s.rect.size.h as f64 * scale).round() as i32);
        let ids: Vec<(String, Option<String>)> =
            s.tabs.iter().map(|t| (label(t), crate::ipc::toplevel_ids(t).0)).collect();
        let key = (w, h, ids, s.shown);
        let buffer = images.entry(key).or_insert_with_key(|(w, h, _, shown)| {
            let pixels = draw(*w, *h, scale, &tabs_of(&s.tabs, (ICON * scale).round() as u32), *shown);
            MemoryRenderBuffer::from_slice(&pixels, Fourcc::Argb8888, (*w, *h), 1, Transform::Normal, None)
        });
        let loc = (s.rect.loc - origin).to_f64().to_physical(scale);
        let src = Rectangle::from_size((w as f64, h as f64).into());
        match MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            loc,
            buffer,
            None,
            Some(src),
            Some(s.rect.size),
            Kind::Unspecified,
        ) {
            Ok(e) => out.push(OutputElement::Image(e)),
            Err(err) => tracing::warn!(?err, "tab strip upload failed"),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(px: &[u8], w: usize, x: usize, y: usize) -> [u8; 4] {
        let i = (y * w + x) * 4;
        [px[i], px[i + 1], px[i + 2], px[i + 3]]
    }
    // In memory: blue, green, red, alpha.
    fn bgra([r, g, b]: [u8; 3]) -> [u8; 4] {
        [b, g, r, 0xFF]
    }
    fn titles(t: &[&str]) -> Vec<Tab> {
        t.iter().map(|s| Tab { title: s.to_string(), icon: None }).collect()
    }

    #[test]
    fn tabs_fit_their_titles_and_the_island_is_centred() {
        let l = layout(600.0, &titles(&["Ghostty", "WhiteRose — VS Code"]));
        assert_eq!(l.tabs.len(), 2);
        let (a, b) = (l.tabs[0].1 - l.tabs[0].0, l.tabs[1].1 - l.tabs[1].0);
        assert!(a < b, "a short title, a short tab ({a} vs {b})");
        assert!((l.island_x + l.island_w / 2.0 - 300.0).abs() <= 1.0, "centred");
        assert_eq!(l.tabs[1].0, l.tabs[0].1 + TAB_GAP);
        // Too many for the tile: they share it, and the island still fits.
        let l = layout(300.0, &titles(&["A very long title one", "A very long title two", "A very long title three"]));
        assert!(l.island_x >= EAR, "{l:?}");
        let w = l.tabs[0].1 - l.tabs[0].0;
        assert!(l.tabs.iter().all(|(x0, x1)| (x1 - x0 - w).abs() < 0.01), "even");
    }

    #[test]
    fn the_island_hangs_from_a_band_and_the_rest_is_see_through() {
        let t = titles(&["Terminal", "Files"]);
        let px = draw(400, 44, 1.0, &t, 1);
        assert_eq!(px.len(), 400 * 44 * 4);
        let l = layout(400.0, &t);
        assert_eq!(pixel(&px, 400, 2, 2), bgra(PANEL), "the band, across the tile");
        assert_eq!(pixel(&px, 400, 2, 20)[3], 0, "beside the island: see-through");
        assert_eq!(pixel(&px, 400, 2, 40)[3], 0, "under it too");
        let mid = (l.island_x + l.island_w / 2.0) as usize;
        assert_eq!(pixel(&px, 400, (l.tabs[0].0 + 3.0) as usize, 32), bgra(PANEL), "the island");
        assert_eq!(pixel(&px, 400, (l.tabs[1].1 - 4.0) as usize, 20), bgra(SHOWN), "the shown tab's pill");
        assert_ne!(pixel(&px, 400, (l.tabs[0].0 + 3.0) as usize, 20), bgra(SHOWN), "only the shown one");
        assert_eq!(pixel(&px, 400, mid, 41)[3], 0, "the gap before the window");
        // An ear: filled right by the band and the island, empty further out.
        let ex = l.island_x as usize - 1;
        assert!(pixel(&px, 400, ex, 7)[3] > 200, "the ear meets the band");
        assert_eq!(pixel(&px, 400, ex - 10, 16)[3], 0, "and curves away");
    }

    #[test]
    fn a_title_that_fits_is_drawn_whole() {
        // Not the same picture as the title cut short with an ellipsis.
        let t = titles(&["Pictures — Files"]);
        let l = layout(600.0, &t);
        let w = text_width("Pictures — Files", FONT_SIZE as f32) as f64;
        assert!(l.tabs[0].1 - l.tabs[0].0 - 2.0 * TAB_PAD >= w + 1.0, "room for it all");
        let (a, b) = (draw(600, 44, 1.0, &t, 0), draw(600, 44, 1.0, &titles(&["Pictures — Fil…"]), 0));
        assert_ne!(a, b);
    }

    #[test]
    fn titles_are_drawn_in_their_tab_and_cut_short() {
        let long = "A very long window title that cannot possibly fit in a tab".to_string();
        let t = vec![Tab { title: long, icon: None }, Tab { title: "B".into(), icon: None }];
        let px = draw(400, 44, 1.0, &t, 1);
        let l = layout(400.0, &t);
        let bright =
            |x0: f64, x1: f64| (8..34).any(|y| (x0 as usize..x1 as usize).any(|x| pixel(&px, 400, x, y)[1] > 0x60));
        assert!(bright(l.tabs[0].0 + TAB_PAD, l.tabs[0].1 - TAB_PAD), "the title shows");
        assert!(!bright(l.tabs[0].1 - TAB_PAD + 1.0, l.tabs[1].0), "and stops at its tab's padding");
    }

    #[test]
    fn an_apps_icon_comes_before_its_title() {
        // A solid red 16 px icon.
        let red = std::sync::Arc::new(crate::app_icons::Icon {
            size: 16,
            bgra: (0..16 * 16).flat_map(|_| [0u8, 0, 255, 255]).collect(),
        });
        let plain = titles(&["Terminal"]);
        let with = vec![Tab { title: "Terminal".into(), icon: Some(red) }];
        let (a, b) = (layout(400.0, &plain), layout(400.0, &with));
        assert_eq!((b.tabs[0].1 - b.tabs[0].0) - (a.tabs[0].1 - a.tabs[0].0), ICON + ICON_GAP, "room for it");
        let px = draw(400, 44, 1.0, &with, 0);
        let (x, y) = ((b.tabs[0].0 + TAB_PAD + 8.0) as usize, (BAND + ISLAND / 2.0) as usize);
        assert_eq!(pixel(&px, 400, x, y), [0, 0, 255, 255], "drawn at the tab's start");
        // Not shown: fainter.
        let px = draw(400, 44, 1.0, &[with[0].clone(), Tab { title: "B".into(), icon: None }], 1);
        let b = layout(400.0, &[with[0].clone(), Tab { title: "B".into(), icon: None }]);
        let p = pixel(&px, 400, (b.tabs[0].0 + TAB_PAD + 8.0) as usize, y);
        assert!(p[2] < 255 && p[2] > 150, "{p:?}");
    }

    #[test]
    fn sharp_at_other_scales() {
        let t = titles(&["Terminal"]);
        let px = draw(800, 88, 2.0, &t, 0);
        assert_eq!(px.len(), 800 * 88 * 4);
        assert_eq!(pixel(&px, 800, 4, 4), bgra(PANEL));
        assert_eq!(pixel(&px, 800, 4, 40)[3], 0);
    }
}
