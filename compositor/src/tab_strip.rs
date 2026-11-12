//! The tab strip above a grouped tile: one tab per window, the shown one
//! lit with an accent underline, each with its title in Geist.
//!
//! The compositor draws it itself: each strip is drawn once into a pixel
//! buffer at the output's real resolution (sharp at 125%, 150%, ...) and
//! kept until its size, titles or shown tab change, so an idle desktop
//! draws nothing new.

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
    /// The tab under `pos`, if it's on this strip.
    pub fn tab_at(&self, pos: Point<f64, Logical>) -> Option<&Window> {
        if !self.rect.to_f64().contains(pos) || self.tabs.is_empty() {
            return None;
        }
        let i = ((pos.x - self.rect.loc.x as f64) * self.tabs.len() as f64 / self.rect.size.w.max(1) as f64) as usize;
        self.tabs.get(i.min(self.tabs.len() - 1))
    }
}

// Colours as (r, g, b): the dark of the bar and island.
const BACKGROUND: [u8; 3] = [0x16, 0x16, 0x18];
const SHOWN: [u8; 3] = [0x2C, 0x2C, 0x30];
const TEXT: [u8; 3] = [0xF5, 0xF5, 0xF7];
const TEXT_DIM: [u8; 3] = [0x8E, 0x8E, 0x93];
const ACCENT: [u8; 3] = [0xE8, 0xA3, 0xAE];
/// Title size and side padding, in logical pixels.
const FONT_SIZE: f64 = 12.5;
const PADDING: f64 = 10.0;

fn font() -> &'static fontdue::Font {
    static FONT: OnceLock<fontdue::Font> = OnceLock::new();
    FONT.get_or_init(|| {
        fontdue::Font::from_bytes(&include_bytes!("../../shell/fonts/Geist-Medium.ttf")[..], Default::default())
            .expect("the bundled Geist font loads")
    })
}

/// Draws a strip `w` x `h` pixels (Argb8888: B, G, R, A in memory) at
/// `scale` pixels per logical pixel.
pub fn draw(w: i32, h: i32, scale: f64, titles: &[String], shown: usize) -> Vec<u8> {
    let (w, h) = (w.max(1) as usize, h.max(1) as usize);
    let mut px = vec![0u8; w * h * 4];
    fill(&mut px, w, 0..w, 0..h, BACKGROUND);
    let n = titles.len().max(1);
    let gap = scale.round().max(1.0) as usize;
    let underline = (2.0 * scale).round().max(1.0) as usize;
    let pad = (PADDING * scale).round() as usize;
    let size = (FONT_SIZE * scale) as f32;
    for (i, title) in titles.iter().enumerate() {
        let x0 = i * w / n;
        let x1 = ((i + 1) * w / n).saturating_sub(if i + 1 < n { gap } else { 0 }).max(x0);
        if i == shown {
            fill(&mut px, w, x0..x1, 0..h, SHOWN);
            fill(&mut px, w, x0..x1, h.saturating_sub(underline)..h, ACCENT);
        }
        let colour = if i == shown { TEXT } else { TEXT_DIM };
        text(&mut px, w, h, x0 + pad, x1.saturating_sub(pad), title, size, colour);
    }
    px
}

fn fill(px: &mut [u8], w: usize, xs: std::ops::Range<usize>, ys: std::ops::Range<usize>, [r, g, b]: [u8; 3]) {
    for y in ys {
        for x in xs.clone() {
            let i = (y * w + x) * 4;
            px[i..i + 4].copy_from_slice(&[b, g, r, 0xFF]);
        }
    }
}

/// Writes `s` between `x0` and `x1`, centred vertically; cut short with
/// an ellipsis if it doesn't fit.
#[allow(clippy::too_many_arguments)]
fn text(px: &mut [u8], w: usize, h: usize, x0: usize, x1: usize, s: &str, size: f32, colour: [u8; 3]) {
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
    let baseline = ((h as f32 - (line.ascent - line.descent)) / 2.0 + line.ascent).round() as i32;
    let mut pen = x0 as f32;
    for c in chars {
        let (m, bitmap) = font.rasterize(c, size);
        let gx = (pen + m.xmin as f32).round() as i32;
        let gy = baseline - m.height as i32 - m.ymin;
        for row in 0..m.height {
            for col in 0..m.width {
                let (x, y) = (gx + col as i32, gy + row as i32);
                if x < x0 as i32 || x >= x1.max(x0) as i32 || y < 0 || y >= h as i32 {
                    continue;
                }
                let a = bitmap[row * m.width + col] as u32;
                let i = (y as usize * w + x as usize) * 4;
                let [r, g, b] = colour;
                for (k, c) in [b, g, r].into_iter().enumerate() {
                    px[i + k] = ((c as u32 * a + px[i + k] as u32 * (255 - a)) / 255) as u8;
                }
            }
        }
        pen += m.advance_width;
    }
}

/// What a drawn strip shows: its size in pixels, the titles, the shown tab.
type StripKey = (i32, i32, Vec<String>, usize);

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
        let titles: Vec<String> = s.tabs.iter().map(label).collect();
        let key = (w, h, titles, s.shown);
        let buffer = images.entry(key).or_insert_with_key(|(w, h, titles, shown)| {
            let pixels = draw(*w, *h, scale, titles, *shown);
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

    #[test]
    fn the_shown_tab_is_lit_and_underlined() {
        let titles = vec!["Terminal".to_string(), "Files".to_string()];
        let px = draw(400, 28, 1.0, &titles, 1);
        assert_eq!(px.len(), 400 * 28 * 4);
        // In memory: blue, green, red, alpha.
        let bgra = |[r, g, b]: [u8; 3]| [b, g, r, 0xFF];
        assert_eq!(pixel(&px, 400, 2, 2), bgra(BACKGROUND), "the first tab: background");
        assert_eq!(pixel(&px, 400, 202, 2), bgra(SHOWN), "the second tab: lit");
        assert_eq!(pixel(&px, 400, 300, 27), bgra(ACCENT), "underlined");
        assert_ne!(pixel(&px, 400, 199, 27), bgra(ACCENT), "only the shown tab");
    }

    #[test]
    fn titles_are_drawn_inside_their_tab() {
        let px = draw(400, 28, 1.0, &["Terminal".to_string(), String::new()], 1);
        // Text is the only light grey or white above the underline.
        let bright = |x0: usize, x1: usize| (0..24).any(|y| (x0..x1).any(|x| pixel(&px, 400, x, y)[1] > 0x60));
        assert!(bright(10, 190), "the title shows");
        assert!(!bright(200, 400), "an empty title draws nothing");
    }

    #[test]
    fn long_titles_stop_at_their_tab() {
        let long = "A very long window title that cannot possibly fit".to_string();
        let px = draw(120, 28, 1.0, &[long, "B".to_string()], 0);
        // Nothing of the first title past its padding (the tab ends at 59).
        for y in 0..26 {
            for x in 50..59 {
                assert_eq!(pixel(&px, 120, x, y)[..3], [SHOWN[2], SHOWN[1], SHOWN[0]], "({x}, {y})");
            }
        }
    }
}
