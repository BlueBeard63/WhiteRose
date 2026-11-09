//! hyprland-toplevel-export-v1: copies one window's pixels into a client's
//! shared-memory buffer. QuickShell's ScreencopyView uses it for live window
//! previews (dock hover, window switcher).
//!
//! Battery: a live view asks for a new frame as soon as it gets one. Copies
//! are therefore held until the window actually redraws, and at most one per
//! window every `MIN_INTERVAL`, so an idle window costs nothing even while its
//! preview is on screen, and a video plays back at ~15 fps rather than 60.

use std::{
    cell::Cell,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant, SystemTime},
};

use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            damage::OutputDamageTracker,
            element::{surface::WaylandSurfaceRenderElement, AsRenderElements},
            gles::{GlesRenderer, GlesTexture},
            Bind, ExportMem, Offscreen, TextureMapping,
        },
    },
    desktop::Window,
    output::Output,
    reexports::{
        calloop::timer::{TimeoutAction, Timer},
        wayland_server::{
            protocol::{wl_buffer::WlBuffer, wl_shm},
            Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
        },
    },
    utils::{Buffer as BufferCoord, Physical, Rectangle, Scale, Size, Transform},
    wayland::shm::with_buffer_contents_mut,
};

use crate::{
    protocols::toplevel_export::{
        hyprland_toplevel_export_frame_v1::{self, HyprlandToplevelExportFrameV1},
        hyprland_toplevel_export_manager_v1::{self, HyprlandToplevelExportManagerV1},
    },
    state::WhiteRose,
};

const VERSION: u32 = 2;
/// At most ~15 preview frames per second per window.
pub const MIN_INTERVAL: Duration = Duration::from_millis(66);

pub struct FrameData {
    window: u64,
    size: Size<i32, Physical>,
    used: AtomicBool,
}

pub struct PendingCopy {
    frame: HyprlandToplevelExportFrameV1,
    buffer: WlBuffer,
    window: u64,
    ignore_damage: bool,
}

/// Per-window capture bookkeeping, kept in the window's user data.
#[derive(Default)]
struct ExportTracking {
    /// The window committed new content since its last copy.
    dirty: Cell<bool>,
    last_copy: Cell<Option<Instant>>,
}

fn tracking(w: &Window) -> &ExportTracking {
    w.user_data().insert_if_missing(ExportTracking::default);
    w.user_data().get::<ExportTracking>().expect("just inserted")
}

pub fn init(dh: &DisplayHandle) {
    dh.create_global::<WhiteRose, HyprlandToplevelExportManagerV1, ()>(VERSION, ());
}

/// The size a capture of `w` will have.
pub fn capture_size(w: &Window, scale: f64) -> Size<i32, Physical> {
    w.geometry().size.to_f64().to_physical_precise_round(scale)
}

/// Checks a client buffer can take a `size` Argb8888/Xrgb8888 frame.
pub fn validate(
    width: i32,
    height: i32,
    stride: i32,
    format: wl_shm::Format,
    size: Size<i32, Physical>,
) -> Result<(), String> {
    if !matches!(format, wl_shm::Format::Argb8888 | wl_shm::Format::Xrgb8888) {
        return Err(format!("unsupported format {format:?}"));
    }
    if (width, height) != (size.w, size.h) {
        return Err(format!("buffer is {width}x{height}, frame is {}x{}", size.w, size.h));
    }
    if stride < width * 4 {
        return Err(format!("stride {stride} is too small for width {width}"));
    }
    Ok(())
}

/// Copies tightly packed rows into a buffer with its own stride.
pub fn copy_rows(src: &[u8], src_stride: usize, dst: &mut [u8], dst_stride: usize, rows: usize) {
    let row = src_stride.min(dst_stride);
    for y in 0..rows {
        dst[y * dst_stride..y * dst_stride + row].copy_from_slice(&src[y * src_stride..y * src_stride + row]);
    }
}

impl WhiteRose {
    fn output_scale(&self) -> f64 {
        self.space.outputs().next().map(|o| o.current_scale().fractional_scale()).unwrap_or(1.0)
    }

    fn start_frame(
        &mut self,
        data_init: &mut DataInit<'_, Self>,
        frame: New<HyprlandToplevelExportFrameV1>,
        window: u64,
    ) {
        let scale = self.output_scale();
        let size = self.workspaces.mru().iter().find(|w| self.window_id(w) == window).map(|w| capture_size(w, scale));
        let frame =
            data_init.init(frame, FrameData { window, size: size.unwrap_or_default(), used: AtomicBool::new(false) });
        match size {
            Some(s) if s.w > 0 && s.h > 0 => {
                frame.buffer(wl_shm::Format::Argb8888, s.w as u32, s.h as u32, s.w as u32 * 4);
                if frame.version() >= 2 {
                    frame.buffer_done();
                }
            }
            // Unknown window, or one that hasn't drawn anything yet.
            _ => frame.failed(),
        }
    }

    /// Called from the compositor commit handler for a toplevel's surface.
    pub fn mark_export_dirty(&mut self, w: &Window) {
        tracking(w).dirty.set(true);
        let id = self.window_id(w);
        if self.pending_exports.iter().any(|p| p.window == id) {
            self.schedule_redraw();
        }
    }

    /// Windows with a copy waiting: they get frame callbacks even when hidden
    /// (another workspace) so their previews stay live.
    pub fn exporting_windows(&self) -> Vec<Window> {
        self.workspaces
            .mru()
            .iter()
            .filter(|w| self.pending_exports.iter().any(|p| p.window == self.window_id(w)))
            .cloned()
            .collect()
    }

    fn arm_export_timer(&mut self, after: Duration) {
        if self.export_timer_armed {
            return;
        }
        self.export_timer_armed = true;
        let _ = self.loop_handle.insert_source(Timer::from_duration(after), |_, _, state| {
            state.export_timer_armed = false;
            state.schedule_redraw();
            TimeoutAction::Drop
        });
    }
}

/// A window rendered offscreen.
struct Capture {
    /// Argb8888, tightly packed.
    pixels: Vec<u8>,
    size: Size<i32, Physical>,
    /// Rows run bottom to top (GL readback); clients are told via y_invert.
    bottom_up: bool,
}

/// Renders one window into Argb8888 pixels.
fn render_window(renderer: &mut GlesRenderer, w: &Window, scale: f64) -> Result<Capture, String> {
    let size = capture_size(w, scale);
    let buffer_size: Size<i32, BufferCoord> = (size.w, size.h).into();
    let mut texture: GlesTexture =
        Offscreen::<GlesTexture>::create_buffer(renderer, Fourcc::Argb8888, buffer_size).map_err(|e| e.to_string())?;
    let mut fb = renderer.bind(&mut texture).map_err(|e| e.to_string())?;
    // Place the window's content (not its shadow margin) at the origin.
    let geo = w.geometry();
    let loc = smithay::utils::Point::<f64, smithay::utils::Logical>::from((-geo.loc.x as f64, -geo.loc.y as f64))
        .to_physical_precise_round(scale);
    let elements: Vec<WaylandSurfaceRenderElement<GlesRenderer>> =
        w.render_elements(renderer, loc, Scale::from(scale), 1.0);
    let mut tracker = OutputDamageTracker::new(size, scale, Transform::Normal);
    tracker.render_output(renderer, &mut fb, 0, &elements, [0.0, 0.0, 0.0, 0.0]).map_err(|e| format!("{e:?}"))?;
    let mapping = renderer
        .copy_framebuffer(&fb, Rectangle::from_size(buffer_size), Fourcc::Argb8888)
        .map_err(|e| e.to_string())?;
    let bottom_up = !mapping.flipped();
    let pixels = renderer.map_texture(&mapping).map_err(|e| e.to_string())?.to_vec();
    Ok(Capture { pixels, size, bottom_up })
}

/// Runs after each frame: fills every copy that is due.
pub fn process(renderer: &mut GlesRenderer, _output: &Output, state: &mut WhiteRose) {
    if state.pending_exports.is_empty() {
        return;
    }
    let scale = state.output_scale();
    let now = Instant::now();
    let mut wait: Option<Duration> = None;
    let pending = std::mem::take(&mut state.pending_exports);
    for p in pending {
        let Some(w) = state.workspaces.mru().iter().find(|w| state.window_id(w) == p.window).cloned() else {
            p.frame.failed();
            continue;
        };
        let t = tracking(&w);
        if !p.ignore_damage && !t.dirty.get() {
            state.pending_exports.push(p); // nothing new to show yet
            continue;
        }
        if let Some(last) = t.last_copy.get() {
            let since = now.duration_since(last);
            if since < MIN_INTERVAL {
                wait = Some(wait.map_or(MIN_INTERVAL - since, |d| d.min(MIN_INTERVAL - since)));
                state.pending_exports.push(p);
                continue;
            }
        }
        match render_window(renderer, &w, scale) {
            Ok(Capture { pixels, size, bottom_up }) => {
                let written = with_buffer_contents_mut(&p.buffer, |ptr, len, data| {
                    validate(data.width, data.height, data.stride, data.format, size)?;
                    let needed = data.offset as usize + data.stride as usize * data.height as usize;
                    if needed > len {
                        return Err("buffer pool too small".to_string());
                    }
                    // Safety: the range was bounds-checked against the pool length above.
                    let dst = unsafe {
                        std::slice::from_raw_parts_mut(ptr.add(data.offset as usize), needed - data.offset as usize)
                    };
                    copy_rows(&pixels, size.w as usize * 4, dst, data.stride as usize, size.h as usize);
                    Ok(())
                });
                match written {
                    Ok(Ok(())) => {
                        t.dirty.set(false);
                        t.last_copy.set(Some(now));
                        if bottom_up {
                            p.frame.flags(hyprland_toplevel_export_frame_v1::Flags::YInvert);
                        }
                        let ts = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
                        p.frame.ready((ts.as_secs() >> 32) as u32, ts.as_secs() as u32, ts.subsec_nanos());
                    }
                    Ok(Err(e)) => {
                        tracing::debug!(e, "toplevel export: bad buffer");
                        p.frame.failed();
                    }
                    Err(e) => {
                        tracing::debug!(?e, "toplevel export: buffer not shm");
                        p.frame.failed();
                    }
                }
            }
            Err(e) => {
                tracing::debug!(e, "toplevel export: render failed");
                p.frame.failed();
            }
        }
    }
    if let Some(d) = wait {
        state.arm_export_timer(d);
    }
}

impl GlobalDispatch<HyprlandToplevelExportManagerV1, ()> for WhiteRose {
    fn bind(
        _state: &mut Self,
        _dh: &DisplayHandle,
        _client: &Client,
        resource: New<HyprlandToplevelExportManagerV1>,
        _: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<HyprlandToplevelExportManagerV1, ()> for WhiteRose {
    fn request(
        state: &mut Self,
        _client: &Client,
        _manager: &HyprlandToplevelExportManagerV1,
        request: hyprland_toplevel_export_manager_v1::Request,
        _: &(),
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        use hyprland_toplevel_export_manager_v1::Request;
        match request {
            // v1 identifies windows by a number; ours are the IPC window ids.
            Request::CaptureToplevel { frame, handle, .. } => state.start_frame(data_init, frame, handle as u64),
            Request::CaptureToplevelWithWlrToplevelHandle { frame, handle, .. } => {
                let window = handle.data::<u64>().copied().unwrap_or(0);
                state.start_frame(data_init, frame, window);
            }
            Request::Destroy => {}
        }
    }
}

impl Dispatch<HyprlandToplevelExportFrameV1, FrameData> for WhiteRose {
    fn request(
        state: &mut Self,
        _client: &Client,
        frame: &HyprlandToplevelExportFrameV1,
        request: hyprland_toplevel_export_frame_v1::Request,
        data: &FrameData,
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        use hyprland_toplevel_export_frame_v1::Request;
        match request {
            Request::Copy { buffer, ignore_damage } => {
                if data.used.swap(true, Ordering::SeqCst) {
                    frame.post_error(hyprland_toplevel_export_frame_v1::Error::AlreadyUsed, "frame already used");
                    return;
                }
                let ok = smithay::wayland::shm::with_buffer_contents(&buffer, |_, _, b| {
                    validate(b.width, b.height, b.stride, b.format, data.size)
                });
                match ok {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        tracing::debug!(e, "toplevel export: rejecting buffer");
                        frame.failed();
                        return;
                    }
                    Err(_) => {
                        // Only shm buffers are supported (no dmabuf event is advertised).
                        frame.failed();
                        return;
                    }
                }
                state.pending_exports.push(PendingCopy {
                    frame: frame.clone(),
                    buffer,
                    window: data.window,
                    ignore_damage: ignore_damage != 0,
                });
                state.schedule_redraw();
            }
            Request::Destroy => state.pending_exports.retain(|p| &p.frame != frame),
        }
    }

    fn destroyed(
        state: &mut Self,
        _client: smithay::reexports::wayland_server::backend::ClientId,
        frame: &HyprlandToplevelExportFrameV1,
        _: &FrameData,
    ) {
        state.pending_exports.retain(|p| &p.frame != frame);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_client_buffers() {
        let size = Size::from((400, 300));
        assert!(validate(400, 300, 1600, wl_shm::Format::Argb8888, size).is_ok());
        assert!(validate(400, 300, 2048, wl_shm::Format::Xrgb8888, size).is_ok(), "padded stride is fine");
        assert!(validate(401, 300, 1604, wl_shm::Format::Argb8888, size).is_err(), "wrong size");
        assert!(validate(400, 300, 1599, wl_shm::Format::Argb8888, size).is_err(), "stride too small");
        assert!(validate(400, 300, 1600, wl_shm::Format::Rgb565, size).is_err(), "wrong format");
    }

    #[test]
    fn copies_rows_into_a_padded_stride() {
        let src = [1, 2, 3, 4, 5, 6, 7, 8]; // 2 rows of 4 bytes
        let mut dst = [0u8; 12]; // 2 rows of 6 bytes
        copy_rows(&src, 4, &mut dst, 6, 2);
        assert_eq!(dst, [1, 2, 3, 4, 0, 0, 5, 6, 7, 8, 0, 0]);
    }
}
