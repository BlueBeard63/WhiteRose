//! The pointer, for backends that draw it themselves (DRM; the nested window
//! leaves it to the host). Named cursors come from the XCursor theme
//! ($XCURSOR_THEME, $XCURSOR_SIZE); clients can also set their own surface.
//!
//! Only the first frame of animated cursors is used: an animated "busy"
//! pointer would mean redrawing the screen many times a second.

use std::collections::HashMap;

use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            element::{
                memory::{MemoryRenderBuffer, MemoryRenderBufferRenderElement},
                surface::{render_elements_from_surface_tree, WaylandSurfaceRenderElement},
                Kind,
            },
            gles::GlesRenderer,
        },
    },
    input::pointer::{CursorIcon, CursorImageAttributes, CursorImageStatus},
    reexports::wayland_server::Resource,
    utils::{Logical, Point, Scale, Transform},
    wayland::compositor,
};

use crate::render::OutputElement;

/// One loaded cursor image: the buffer (at `scale`) and its hotspot in
/// logical pixels.
#[derive(Clone)]
struct Loaded {
    buffer: MemoryRenderBuffer,
    hotspot: Point<f64, Logical>,
}

pub struct Cursors {
    theme: xcursor::CursorTheme,
    size: u32,
    /// By (name, integer scale); None when the theme doesn't have it.
    cache: HashMap<(&'static str, u32), Option<Loaded>>,
}

impl Cursors {
    pub fn from_env() -> Self {
        let theme = std::env::var("XCURSOR_THEME").unwrap_or_else(|_| "default".into());
        let size = std::env::var("XCURSOR_SIZE").ok().and_then(|s| s.parse().ok()).unwrap_or(24);
        Self { theme: xcursor::CursorTheme::load(&theme), size, cache: HashMap::new() }
    }

    fn named(&mut self, icon: CursorIcon, scale: u32) -> Option<Loaded> {
        let names = std::iter::once(icon.name()).chain(icon.alt_names().iter().copied());
        for name in names.chain(["default", "left_ptr"]) {
            let key = (name, scale);
            if !self.cache.contains_key(&key) {
                let loaded = load(&self.theme, name, self.size * scale, scale);
                self.cache.insert(key, loaded);
            }
            if let Some(Some(l)) = self.cache.get(&key) {
                return Some(l.clone());
            }
        }
        None
    }

    /// The pointer's render elements for an output, `at` in output-local
    /// logical coordinates. Empty when hidden. A client cursor surface that
    /// has gone away falls back to the default arrow.
    pub fn elements(
        &mut self,
        renderer: &mut GlesRenderer,
        status: &mut CursorImageStatus,
        at: Point<f64, Logical>,
        scale: f64,
    ) -> Vec<OutputElement> {
        if let CursorImageStatus::Surface(s) = status {
            if !s.is_alive() {
                *status = CursorImageStatus::default_named();
            }
        }
        match status {
            CursorImageStatus::Hidden => Vec::new(),
            CursorImageStatus::Surface(surface) => {
                let hotspot = compositor::with_states(surface, |states| {
                    states
                        .data_map
                        .get::<std::sync::Mutex<CursorImageAttributes>>()
                        .map(|a| a.lock().unwrap().hotspot)
                        .unwrap_or_default()
                });
                let loc = (at - hotspot.to_f64()).to_physical_precise_round(scale);
                render_elements_from_surface_tree::<_, WaylandSurfaceRenderElement<_>>(
                    renderer,
                    surface,
                    loc,
                    Scale::from(scale),
                    1.0,
                    Kind::Cursor,
                )
                .into_iter()
                .map(OutputElement::Surface)
                .collect()
            }
            CursorImageStatus::Named(icon) => {
                let Some(l) = self.named(*icon, scale.ceil().max(1.0) as u32) else { return Vec::new() };
                let loc = (at - l.hotspot).to_physical(scale);
                match MemoryRenderBufferRenderElement::from_buffer(
                    renderer,
                    loc,
                    &l.buffer,
                    None,
                    None,
                    None,
                    Kind::Cursor,
                ) {
                    Ok(e) => vec![OutputElement::Cursor(e)],
                    Err(err) => {
                        tracing::warn!(?err, "cursor upload failed");
                        Vec::new()
                    }
                }
            }
        }
    }
}

/// The image nearest `pixels` in size from the cursor `name`, as a buffer
/// at integer `scale`.
fn load(theme: &xcursor::CursorTheme, name: &str, pixels: u32, scale: u32) -> Option<Loaded> {
    let path = theme.load_icon(name)?;
    let data = std::fs::read(path).ok()?;
    let images = xcursor::parser::parse_xcursor(&data)?;
    let best = images.iter().min_by_key(|i| (i.size as i64 - pixels as i64).abs())?.size;
    let image = images.into_iter().find(|i| i.size == best)?;
    let buffer = MemoryRenderBuffer::from_slice(
        &image.pixels_rgba,
        Fourcc::Argb8888,
        (image.width as i32, image.height as i32),
        scale as i32,
        Transform::Normal,
        None,
    );
    let hotspot = Point::from((image.xhot as f64 / scale as f64, image.yhot as f64 / scale as f64));
    Some(Loaded { buffer, hotspot })
}
