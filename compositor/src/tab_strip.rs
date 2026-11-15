//! The tab strip above a grouped tile, as a small island (Figma: "Grouping
//! — tab strip options", C): a thin black band across the top of the tile
//! (a little below it, clear of the bar's island; its ends rounded)
//! with the tabs hanging from it in the middle on concave ears, as the
//! island hangs from the bar. Each tab is its title; the shown one sits on
//! a lighter pill. Beside the island the strip is see-through.
//!
//! The compositor draws it itself: each strip is drawn once into a pixel
//! buffer at the output's real resolution (sharp at 125%, 150%, ...) and
//! kept until its size, titles or shown tab change, so an idle desktop
//! draws nothing new. `layout` places the tabs for both drawing and clicks.
//!
//! It moves when the group changes (`ANIM`, eased out): a new group's band
//! grows from the middle and its island drops out from under it; a tab
//! joining or leaving widens or narrows the island, the others sliding to
//! their places and the new one fading in (the shown tab's pill slides
//! too); a group that ends goes the way it came, drawn over its window
//! while it does. Only then are strips drawn each frame.

use std::{
    cell::RefCell,
    collections::HashMap,
    sync::OnceLock,
    time::{Duration, Instant},
};

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
    /// Moving into how it is now, if it's changed lately.
    pub anim: Option<Anim>,
}

/// How long a strip takes to appear, change or go.
pub const ANIM: Duration = Duration::from_millis(220);

/// A strip changing: since when, and what it was (None: it's new).
#[derive(Debug, Clone)]
pub struct Anim {
    pub start: Instant,
    pub from: Option<(Vec<Window>, usize)>,
}

/// A strip whose group has ended, going away (drawn over its window).
#[derive(Debug, Clone)]
pub struct Leaving {
    pub output: Output,
    pub rect: Rectangle<i32, Logical>,
    pub labels: Vec<Label>,
    pub shown: usize,
    pub start: Instant,
}

/// How far along something started at `start` is at `now`, eased out
/// (quick to start, gentle to settle): 0 to 1.
pub fn eased(start: Instant, now: Instant) -> f64 {
    let p = (now.saturating_duration_since(start).as_secs_f64() / ANIM.as_secs_f64()).clamp(0.0, 1.0);
    1.0 - (1.0 - p).powi(3)
}

impl Strip {
    /// The middle of tab `i` (global coordinates).
    pub fn tab_middle(&self, i: usize) -> Option<Point<f64, Logical>> {
        let (x0, x1) = *layout(self.rect.size.w as f64, &tabs_of(&self.tabs, ICON as u32)).tabs.get(i)?;
        Some((self.rect.loc.x as f64 + (x0 + x1) / 2.0, self.rect.loc.y as f64 + HANG + ISLAND / 2.0).into())
    }

    /// The tab under `pos`, if it's on this strip's island.
    pub fn tab_at(&self, pos: Point<f64, Logical>) -> Option<&Window> {
        if !self.rect.to_f64().contains(pos) || self.tabs.is_empty() {
            return None;
        }
        let l = layout(self.rect.size.w as f64, &tabs_of(&self.tabs, ICON as u32));
        let (x, y) = (pos.x - self.rect.loc.x as f64, pos.y - self.rect.loc.y as f64);
        if !(HANG..HANG + ISLAND).contains(&y) {
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
/// Room above the band, so it doesn't run into the island hanging from
/// the bar (it reaches 14 px below the bar, the tiles start 8 px below:
/// this leaves 8 px between them, as between tiles); the band; where the
/// island starts (under the band).
pub const TOP: f64 = 14.0;
pub const BAND: f64 = 6.0;
pub const HANG: f64 = TOP + BAND;
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

/// What a tab says: its title and its app (for the icon). Kept from the
/// last frame drawn, for a strip whose windows have gone.
#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    pub title: String,
    pub app: Option<String>,
}

fn label_of(w: &Window) -> Label {
    let (app, _) = crate::ipc::toplevel_ids(w);
    Label { title: label(w), app }
}

/// The tabs for `labels`, icons `px` pixels big.
fn tabs_from(labels: &[Label], px: u32) -> Vec<Tab> {
    labels
        .iter()
        .map(|l| Tab { title: l.title.clone(), icon: l.app.as_deref().and_then(|a| crate::app_icons::icon(a, px)) })
        .collect()
}

/// The tabs for `windows`, icons `px` pixels big.
fn tabs_of(windows: &[Window], px: u32) -> Vec<Tab> {
    tabs_from(&windows.iter().map(label_of).collect::<Vec<_>>(), px)
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

/// Where everything is at one moment (logical pixels): the band's ends,
/// the island's left edge and width, how far it has dropped from under
/// the band (0: all the way; -ISLAND: tucked up), each tab drawn (where,
/// how strongly, which of the tabs given), and the shown tab's pill.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub band: (f64, f64),
    pub island: (f64, f64),
    pub drop: f64,
    pub tabs: Vec<(f64, f64, f64, usize)>,
    pub pill: Option<(f64, f64)>,
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// A strip `w` wide, still.
pub fn still(w: f64, l: &StripLayout, shown: usize) -> Scene {
    Scene {
        band: (0.0, w),
        island: (l.island_x, l.island_w),
        drop: 0.0,
        tabs: l.tabs.iter().enumerate().map(|(i, (x0, x1))| (*x0, *x1, 1.0, i)).collect(),
        pill: l.tabs.get(shown).copied(),
    }
}

/// `s` appearing, `t` of the way (0 to 1): the band grows from the middle,
/// then the island drops out from under it, its tabs fading in. Going
/// away is the same backwards.
pub fn appearing(s: &Scene, t: f64) -> Scene {
    let mid = (s.band.0 + s.band.1) / 2.0;
    let grow = (t / 0.6).min(1.0);
    let drop = ((t - 0.3) / 0.7).clamp(0.0, 1.0);
    Scene {
        band: (lerp(mid, s.band.0, grow), lerp(mid, s.band.1, grow)),
        island: s.island,
        drop: -ISLAND * (1.0 - drop),
        tabs: s.tabs.iter().map(|&(x0, x1, a, i)| (x0, x1, a * drop, i)).collect(),
        pill: s.pill,
    }
}

/// From `from` (tabs `old`) to `to` (tabs `new`), `t` of the way: the
/// island's width and every tab still there slide; a tab gone fades out
/// where it was in the first half (its index is then new.len() + its old
/// index), a new one fades in where it'll be once the others have mostly
/// moved, so they never cross; the pill slides to the shown tab.
pub fn changing<W: PartialEq>(from: &Scene, old: &[W], to: &Scene, new: &[W], t: f64) -> Scene {
    let mut tabs = vec![];
    for &(x0, x1, _, i) in &to.tabs {
        match old.iter().position(|w| Some(w) == new.get(i)).and_then(|j| from.tabs.iter().find(|f| f.3 == j)) {
            Some(&(f0, f1, _, _)) => tabs.push((lerp(f0, x0, t), lerp(f1, x1, t), 1.0, i)),
            None => tabs.push((x0, x1, ((t - 0.4) / 0.6).clamp(0.0, 1.0), i)),
        }
    }
    for &(x0, x1, _, j) in &from.tabs {
        if !old.get(j).is_some_and(|w| new.contains(w)) {
            tabs.push((x0, x1, (1.0 - t / 0.5).clamp(0.0, 1.0), new.len() + j));
        }
    }
    let pill = match (from.pill, to.pill) {
        (Some(a), Some(b)) => Some((lerp(a.0, b.0, t), lerp(a.1, b.1, t))),
        (_, b) => b,
    };
    Scene {
        band: to.band,
        island: (lerp(from.island.0, to.island.0, t), lerp(from.island.1, to.island.1, t)),
        drop: 0.0,
        tabs,
        pill,
    }
}

/// True if (x, y) (logical) is on the island's body (dropped `s.drop`)
/// or its ears; the band is drawn over it after.
fn in_body(x: f64, y: f64, s: &Scene) -> bool {
    let top = HANG + s.drop;
    let (bx0, bx1) = (s.island.0, s.island.0 + s.island.1);
    if in_round_rect(x, y, (bx0, top, bx1, top + ISLAND), 0.0, CORNER) {
        return true;
    }
    // The ears: under the band, outside a circle in the corner they fill.
    let ear = |cx: f64| (top..top + EAR).contains(&y) && (x - cx).powi(2) + (y - (top + EAR)).powi(2) >= EAR * EAR;
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
/// memory) at `scale` pixels per logical pixel, still.
pub fn draw(w: i32, h: i32, scale: f64, tabs: &[Tab], shown: usize) -> Vec<u8> {
    let lw = w.max(1) as f64 / scale;
    draw_scene(w, h, scale, tabs, Some(shown), &still(lw, &layout(lw, tabs), shown))
}

/// Draws `scene`: tabs[i] for a scene tab with index i, `shown` lit.
pub fn draw_scene(w: i32, h: i32, scale: f64, tabs: &[Tab], shown: Option<usize>, scene: &Scene) -> Vec<u8> {
    let (w, h) = (w.max(1) as usize, h.max(1) as usize);
    let mut px = vec![0u8; w * h * 4];
    let pill_top = HANG + scene.drop + (ISLAND - PILL) / 2.0 - 1.0;
    let pill = scene.pill.map(|(x0, x1)| (x0, pill_top, x1, pill_top + PILL));
    // Only the rows the island covers need looking at.
    let rows = (((HANG + ISLAND) * scale).ceil() as usize + 1).min(h);
    for y in 0..rows {
        for x in 0..w {
            let i = (y * w + x) * 4;
            let a = coverage(x, y, scale, |lx, ly| in_body(lx, ly, scene));
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
    let (ty0, ty1) = ((pill_top * scale).round() as isize, ((pill_top + PILL) * scale).round() as isize);
    for &(x0, x1, alpha, i) in &scene.tabs {
        let Some(tab) = tabs.get(i) else { continue };
        let lit = shown == Some(i);
        let alpha = (alpha.clamp(0.0, 1.0) * 255.0) as u32;
        let colour = if lit { TEXT } else { TEXT_DIM };
        let mut start = x0 + TAB_PAD;
        if let Some(icon) = &tab.icon {
            let iy = ((pill_top + (PILL - ICON) / 2.0) * scale).round() as isize;
            blit(
                &mut px,
                w,
                h,
                (start * scale).round() as usize,
                iy,
                icon,
                if lit { alpha } else { alpha * ICON_DIM / 255 },
            );
            start += ICON + ICON_GAP;
        }
        let (tx0, tx1) = ((start * scale).round() as usize, ((x1 - TAB_PAD) * scale).round() as usize);
        text(&mut px, w, (ty0, ty1), tx0, tx1, &tab.title, size, colour, alpha);
    }
    // Above the band nothing; the band over all (what's tucked up hides
    // behind it). Its ends rounded on top, as in the design.
    let band = (((HANG) * scale).ceil() as usize).min(h);
    px[..band * w * 4].fill(0);
    for y in 0..band {
        for x in 0..w {
            let a = coverage(x, y, scale, |lx, ly| {
                in_round_rect(lx, ly, (scene.band.0, TOP, scene.band.1, HANG), BAND, 0.0)
            });
            if a > 0 {
                paint(&mut px, (y * w + x) * 4, PANEL, a);
            }
        }
    }
    px
}

/// Draws `icon` (premultiplied) with its top left at (x, y), at `alpha`.
fn blit(px: &mut [u8], w: usize, h: usize, x: usize, y: isize, icon: &crate::app_icons::Icon, alpha: u32) {
    let n = icon.size as usize;
    for iy in 0..n {
        let dy = y + iy as isize;
        if dy < 0 || dy >= h as isize {
            continue;
        }
        for ix in 0..n.min(w.saturating_sub(x)) {
            let s = (iy * n + ix) * 4;
            let d = ((dy as usize) * w + x + ix) * 4;
            let a = icon.bgra[s + 3] as u32 * alpha / 255;
            for k in 0..4 {
                let src = icon.bgra[s + k] as u32 * alpha / 255;
                px[d + k] = (src + px[d + k] as u32 * (255 - a) / 255).min(255) as u8;
            }
        }
    }
}

/// Writes `s` between `x0` and `x1`, centred vertically, at `alpha`; cut
/// short with an ellipsis if it doesn't fit.
#[allow(clippy::too_many_arguments)]
fn text(
    px: &mut [u8],
    w: usize,
    (top, bottom): (isize, isize),
    x0: usize,
    x1: usize,
    s: &str,
    size: f32,
    colour: [u8; 3],
    alpha: u32,
) {
    let h = (bottom - top).max(0);
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
                if x < x0 as i32 || x >= x1.max(x0) as i32 || y < top.max(0) as i32 || y >= bottom as i32 {
                    continue;
                }
                let a = bitmap[row * m.width + col] as u32 * alpha / 255;
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
    /// What each tab drawn lately said (for strips going away).
    labels: RefCell<Vec<(Window, Label)>>,
}

impl StripImages {
    /// The labels last drawn for `windows` (a window never drawn: "Window").
    pub fn labels_of(&self, windows: &[Window]) -> Vec<Label> {
        let known = self.labels.borrow();
        windows
            .iter()
            .map(|w| {
                known
                    .iter()
                    .find(|(k, _)| k == w)
                    .map(|(_, l)| l.clone())
                    .unwrap_or(Label { title: "Window".into(), app: None })
            })
            .collect()
    }

    fn remember(&self, w: &Window, l: &Label) {
        let mut known = self.labels.borrow_mut();
        known.retain(|(k, _)| k != w);
        known.push((w.clone(), l.clone()));
        if known.len() > 64 {
            known.remove(0);
        }
    }
}

/// A window's tab label: its title, else its app id.
fn label(w: &Window) -> String {
    let (app, title) = crate::ipc::toplevel_ids(w);
    title.filter(|t| !t.is_empty()).or(app).unwrap_or_else(|| "Window".into())
}

/// A strip's picture as a render element at `rect` (global).
fn element(
    renderer: &mut GlesRenderer,
    origin: Point<i32, Logical>,
    scale: f64,
    rect: Rectangle<i32, Logical>,
    (w, h): (i32, i32),
    buffer: &MemoryRenderBuffer,
) -> Option<OutputElement> {
    let loc = (rect.loc - origin).to_f64().to_physical(scale);
    let src = Rectangle::from_size((w as f64, h as f64).into());
    match MemoryRenderBufferRenderElement::from_buffer(
        renderer,
        loc,
        buffer,
        None,
        Some(src),
        Some(rect.size),
        Kind::Unspecified,
    ) {
        Ok(e) => Some(OutputElement::Image(e)),
        Err(err) => {
            tracing::warn!(?err, "tab strip upload failed");
            None
        }
    }
}

fn buffer(pixels: &[u8], (w, h): (i32, i32)) -> MemoryRenderBuffer {
    MemoryRenderBuffer::from_slice(pixels, Fourcc::Argb8888, (w, h), 1, Transform::Normal, None)
}

/// The tab strips on `output`, ready to draw (behind the windows).
pub fn elements(renderer: &mut GlesRenderer, output: &Output, state: &WhiteRose) -> Vec<OutputElement> {
    let scale = output.current_scale().fractional_scale();
    let Some(origin) = state.space.output_geometry(output).map(|g| g.loc) else { return Vec::new() };
    let strips = &state.tiling.strip_images;
    let now = Instant::now();
    let mut out = Vec::new();
    for s in state.tiling.strips.iter().filter(|s| s.output == *output) {
        let size = ((s.rect.size.w as f64 * scale).round() as i32, (s.rect.size.h as f64 * scale).round() as i32);
        let labels: Vec<Label> = s.tabs.iter().map(label_of).collect();
        for (w, l) in s.tabs.iter().zip(&labels) {
            strips.remember(w, l);
        }
        let px = (ICON * scale).round() as u32;
        let lw = s.rect.size.w as f64;
        // Moving: drawn afresh each frame.
        if let Some(a) = s.anim.as_ref().filter(|a| now < a.start + ANIM) {
            let t = eased(a.start, now);
            let tabs = tabs_from(&labels, px);
            let to = still(lw, &layout(lw, &tabs), s.shown);
            let (scene, all) = match &a.from {
                None => (appearing(&to, t), tabs),
                Some((old, old_shown)) => {
                    let old_tabs = tabs_from(&strips.labels_of(old), px);
                    let from = still(lw, &layout(lw, &old_tabs), *old_shown);
                    let mut all = tabs;
                    all.extend(old_tabs);
                    (changing(&from, old, &to, &s.tabs, t), all)
                }
            };
            let pixels = draw_scene(size.0, size.1, scale, &all, Some(s.shown), &scene);
            out.extend(element(renderer, origin, scale, s.rect, size, &buffer(&pixels, size)));
            continue;
        }
        let mut images = strips.images.borrow_mut();
        // Titles change; old pictures go now and then.
        if images.len() > 64 {
            images.clear();
        }
        let key = (size.0, size.1, labels.iter().map(|l| (l.title.clone(), l.app.clone())).collect(), s.shown);
        let b = images
            .entry(key)
            .or_insert_with(|| buffer(&draw(size.0, size.1, scale, &tabs_from(&labels, px), s.shown), size));
        out.extend(element(renderer, origin, scale, s.rect, size, b));
    }
    out
}

/// Strips going away on `output` (drawn over the windows).
pub fn leaving_elements(renderer: &mut GlesRenderer, output: &Output, state: &WhiteRose) -> Vec<OutputElement> {
    let scale = output.current_scale().fractional_scale();
    let Some(origin) = state.space.output_geometry(output).map(|g| g.loc) else { return Vec::new() };
    let now = Instant::now();
    let mut out = Vec::new();
    for l in state.tiling.leaving.iter().filter(|l| l.output == *output && now < l.start + ANIM) {
        let size = ((l.rect.size.w as f64 * scale).round() as i32, (l.rect.size.h as f64 * scale).round() as i32);
        let lw = l.rect.size.w as f64;
        let tabs = tabs_from(&l.labels, (ICON * scale).round() as u32);
        let scene = appearing(&still(lw, &layout(lw, &tabs), l.shown), 1.0 - eased(l.start, now));
        let pixels = draw_scene(size.0, size.1, scale, &tabs, Some(l.shown), &scene);
        out.extend(element(renderer, origin, scale, l.rect, size, &buffer(&pixels, size)));
    }
    out
}

/// Whether a strip is moving now (frames keep coming until it stops).
pub fn animating(state: &WhiteRose) -> bool {
    let now = Instant::now();
    state.tiling.strips.iter().any(|s| s.anim.as_ref().is_some_and(|a| now < a.start + ANIM))
        || state.tiling.leaving.iter().any(|l| now < l.start + ANIM)
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
        let px = draw(400, 58, 1.0, &t, 1);
        assert_eq!(px.len(), 400 * 58 * 4);
        let l = layout(400.0, &t);
        assert_eq!(pixel(&px, 400, 20, 16), bgra(PANEL), "the band, across the tile");
        assert_eq!(pixel(&px, 400, 20, 8)[3], 0, "below the top, clear of the bar's island");
        assert_eq!(pixel(&px, 400, 0, 14)[3], 0, "its ends rounded");
        assert_eq!(pixel(&px, 400, 2, 34)[3], 0, "beside the island: see-through");
        assert_eq!(pixel(&px, 400, 2, 54)[3], 0, "under it too");
        let mid = (l.island_x + l.island_w / 2.0) as usize;
        assert_eq!(pixel(&px, 400, (l.tabs[0].0 + 3.0) as usize, 46), bgra(PANEL), "the island");
        assert_eq!(pixel(&px, 400, (l.tabs[1].1 - 4.0) as usize, 34), bgra(SHOWN), "the shown tab's pill");
        assert_ne!(pixel(&px, 400, (l.tabs[0].0 + 3.0) as usize, 34), bgra(SHOWN), "only the shown one");
        assert_eq!(pixel(&px, 400, mid, 55)[3], 0, "the gap before the window");
        // An ear: filled right by the band and the island, empty further out.
        let ex = l.island_x as usize - 1;
        assert!(pixel(&px, 400, ex, 21)[3] > 200, "the ear meets the band");
        assert_eq!(pixel(&px, 400, ex - 10, 30)[3], 0, "and curves away");
    }

    #[test]
    fn a_title_that_fits_is_drawn_whole() {
        // Not the same picture as the title cut short with an ellipsis.
        let t = titles(&["Pictures - Files"]);
        let l = layout(600.0, &t);
        let w = text_width("Pictures - Files", FONT_SIZE as f32) as f64;
        assert!(l.tabs[0].1 - l.tabs[0].0 - 2.0 * TAB_PAD >= w + 1.0, "room for it all");
        let (a, b) = (draw(600, 58, 1.0, &t, 0), draw(600, 58, 1.0, &titles(&["Pictures — Fil…"]), 0));
        assert_ne!(a, b);
    }

    #[test]
    fn titles_are_drawn_in_their_tab_and_cut_short() {
        let long = "A very long window title that cannot possibly fit in a tab".to_string();
        let t = vec![Tab { title: long, icon: None }, Tab { title: "B".into(), icon: None }];
        let px = draw(400, 58, 1.0, &t, 1);
        let l = layout(400.0, &t);
        let bright =
            |x0: f64, x1: f64| (22..48).any(|y| (x0 as usize..x1 as usize).any(|x| pixel(&px, 400, x, y)[1] > 0x60));
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
        let px = draw(400, 58, 1.0, &with, 0);
        let (x, y) = ((b.tabs[0].0 + TAB_PAD + 8.0) as usize, (HANG + ISLAND / 2.0) as usize);
        assert_eq!(pixel(&px, 400, x, y), [0, 0, 255, 255], "drawn at the tab's start");
        // Not shown: fainter.
        let px = draw(400, 58, 1.0, &[with[0].clone(), Tab { title: "B".into(), icon: None }], 1);
        let b = layout(400.0, &[with[0].clone(), Tab { title: "B".into(), icon: None }]);
        let p = pixel(&px, 400, (b.tabs[0].0 + TAB_PAD + 8.0) as usize, y);
        assert!(p[2] < 255 && p[2] > 150, "{p:?}");
    }

    #[test]
    fn a_new_strip_grows_from_the_middle_and_drops_out() {
        let t = titles(&["Terminal", "Files"]);
        let end = still(400.0, &layout(400.0, &t), 1);
        let start = appearing(&end, 0.0);
        assert_eq!(start.band, (200.0, 200.0), "nothing of the band yet");
        assert_eq!(start.drop, -ISLAND, "the island tucked up");
        assert!(start.tabs.iter().all(|t| t.2 == 0.0), "its tabs not there");
        assert_eq!(appearing(&end, 1.0), end, "and at the end, as it stays");
        let mid = appearing(&end, 0.5);
        assert!(mid.band.0 < 200.0 && mid.drop < 0.0 && mid.drop > -ISLAND, "{mid:?}");
        // Drawn: at the start there's nothing of it; the band hides the island.
        let px = draw_scene(400, 58, 1.0, &t, Some(1), &start);
        assert!(px.iter().skip(3).step_by(4).all(|a| *a == 0), "nothing drawn");
    }

    #[test]
    fn tabs_slide_fade_in_and_out_as_the_group_changes() {
        let (a, b, c) = ("a", "b", "c");
        let old_tabs = titles(&["Terminal", "Files"]);
        let new_tabs = titles(&["Terminal", "Notes", "A much longer title"]);
        let from = still(600.0, &layout(600.0, &old_tabs), 1);
        let to = still(600.0, &layout(600.0, &new_tabs), 2);
        // "b" left, "c" joined; "a" stayed.
        let (old, new) = (vec![a, b], vec![a, c, "d"]);
        let at = |t| changing(&from, &old, &to, &new, t);
        let start = at(0.0);
        assert_eq!(start.island, from.island, "starts where it was");
        assert_eq!((start.tabs[0].0, start.tabs[0].1), (from.tabs[0].0, from.tabs[0].1), "\"a\" where it was");
        assert_eq!(start.tabs[1].2, 0.0, "the new ones not there yet");
        let gone = start.tabs.iter().find(|t| t.3 == new.len() + 1).expect("\"b\" fading out");
        assert_eq!(gone.2, 1.0);
        assert_eq!(start.pill, from.pill, "the pill where it was");
        let end = at(1.0);
        assert_eq!(end.island, to.island);
        assert_eq!((end.tabs[0].0, end.tabs[0].1), (to.tabs[0].0, to.tabs[0].1));
        assert!(end.tabs.iter().filter(|t| t.3 < new.len()).all(|t| t.2 == 1.0), "the new ones there");
        assert_eq!(end.tabs.iter().find(|t| t.3 == new.len() + 1).unwrap().2, 0.0, "the gone one gone");
        assert_eq!(end.pill, to.pill, "the pill on the shown tab");
        let mid = at(0.5);
        assert!(mid.island.1 > from.island.1 && mid.island.1 < to.island.1, "the island widening");
    }

    #[test]
    fn eases_out_over_the_animation() {
        let s = Instant::now();
        assert_eq!(eased(s, s), 0.0);
        assert!(eased(s, s + ANIM / 2) > 0.5, "quick to start");
        assert_eq!(eased(s, s + ANIM), 1.0);
        assert_eq!(eased(s, s + ANIM * 3), 1.0);
    }

    #[test]
    fn sharp_at_other_scales() {
        let t = titles(&["Terminal"]);
        let px = draw(800, 116, 2.0, &t, 0);
        assert_eq!(px.len(), 800 * 116 * 4);
        assert_eq!(pixel(&px, 800, 40, 34), bgra(PANEL), "the band, at twice the pixels");
        assert_eq!(pixel(&px, 800, 4, 56)[3], 0);
    }
}
