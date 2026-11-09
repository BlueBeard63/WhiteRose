//! Screenshots. PrtSc captures the whole output: the next frame is drawn
//! into an offscreen texture (exactly what is on screen), read back, then a
//! worker thread encodes and saves the PNG so a large capture never stalls the
//! compositor. When it's done the image goes on the clipboard and the shell is
//! told the path, so it can show the "Screenshot saved" toast.

use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use chrono::{DateTime, Local};
use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            damage::OutputDamageTracker,
            gles::{GlesRenderer, GlesTexture},
            Bind, ExportMem, Offscreen, TextureMapping,
        },
    },
    output::Output,
    utils::{Buffer as BufferCoord, Rectangle, Size, Transform},
    wayland::selection::data_device::set_data_device_selection,
};

use crate::{ipc, render, state::WhiteRose};

/// A finished capture, reported back to the main loop.
#[derive(Debug)]
pub enum Outcome {
    Saved { path: PathBuf, width: u32, height: u32, png: Arc<Vec<u8>> },
    Failed(String),
}

/// Reverses row order in place. OpenGL reads framebuffers bottom-up.
pub fn flip_rows(pixels: &mut [u8], stride: usize) {
    if stride == 0 {
        return;
    }
    let rows = pixels.len() / stride;
    for i in 0..rows / 2 {
        let (top, bottom) = pixels.split_at_mut((rows - 1 - i) * stride);
        top[i * stride..(i + 1) * stride].swap_with_slice(&mut bottom[..stride]);
    }
}

/// "Screenshot 2026-11-09 14-32-07.png" (local time; no colons, which some
/// file managers and other OSes dislike).
pub fn file_name(now: DateTime<Local>) -> String {
    now.format("Screenshot %Y-%m-%d %H-%M-%S.png").to_string()
}

/// `<Pictures>/Screenshots`, where Pictures comes from XDG_PICTURES_DIR in
/// user-dirs.dirs (which may use $HOME), defaulting to ~/Pictures.
pub fn screenshots_dir_from(home: &Path, user_dirs: Option<&str>) -> PathBuf {
    let pictures = user_dirs
        .and_then(|text| {
            text.lines().map(str::trim).filter(|l| !l.starts_with('#')).find_map(|l| {
                let value = l.strip_prefix("XDG_PICTURES_DIR=")?.trim().trim_matches('"');
                let value = value.replace("$HOME", &home.to_string_lossy());
                // Only absolute paths are valid in user-dirs.dirs.
                Some(PathBuf::from(value)).filter(|p| p.is_absolute())
            })
        })
        .unwrap_or_else(|| home.join("Pictures"));
    pictures.join("Screenshots")
}

pub fn screenshots_dir() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"));
    let config = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".config"));
    let user_dirs = std::fs::read_to_string(config.join("user-dirs.dirs")).ok();
    screenshots_dir_from(&home, user_dirs.as_deref())
}

/// `dir/name`, or `dir/<stem> (2).png` etc. if that file already exists
/// (two captures in the same second must not overwrite each other).
pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, "png"));
    (2..).map(|n| dir.join(format!("{stem} ({n}).{ext}"))).find(|p| !p.exists()).expect("unbounded")
}

pub fn encode_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    if rgba.len() != width as usize * height as usize * 4 {
        return Err(format!("pixel buffer is {} bytes, expected {}x{}x4", rgba.len(), width, height));
    }
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, width, height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        // Fast compression: screenshots are saved immediately, size matters less.
        enc.set_compression(png::Compression::Fast);
        let mut writer = enc.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(rgba).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

/// Runs on the worker thread.
pub fn encode_and_save(rgba: &[u8], width: u32, height: u32, dir: &Path, now: DateTime<Local>) -> Outcome {
    let result = (|| {
        let png = encode_png(rgba, width, height)?;
        std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        let path = unique_path(dir, &file_name(now));
        let mut f = std::fs::File::create_new(&path).map_err(|e| format!("create {}: {e}", path.display()))?;
        f.write_all(&png).map_err(|e| format!("write {}: {e}", path.display()))?;
        Ok(Outcome::Saved { path, width, height, png: Arc::new(png) })
    })();
    result.unwrap_or_else(Outcome::Failed)
}

/// Draws the output offscreen and returns RGBA pixels, top row first.
fn capture(renderer: &mut GlesRenderer, output: &Output, state: &WhiteRose) -> Result<(Vec<u8>, u32, u32), String> {
    let mode = output.current_mode().ok_or("output has no mode")?;
    let size = mode.size;
    let buffer_size: Size<i32, BufferCoord> = (size.w, size.h).into();

    let mut texture: GlesTexture =
        Offscreen::<GlesTexture>::create_buffer(renderer, Fourcc::Abgr8888, buffer_size).map_err(|e| e.to_string())?;
    let mut framebuffer = renderer.bind(&mut texture).map_err(|e| e.to_string())?;
    // A fresh tracker means a full redraw, independent of on-screen damage.
    let mut tracker = OutputDamageTracker::new(size, output.current_scale().fractional_scale(), Transform::Normal);
    render::compose(renderer, &mut framebuffer, output, state, &mut tracker, 0)?;

    let mapping = renderer
        .copy_framebuffer(&framebuffer, Rectangle::from_size(buffer_size), Fourcc::Abgr8888)
        .map_err(|e| e.to_string())?;
    // GL reads bottom row first. Smithay reports `flipped()` relative to the
    // texture's own orientation, which for an offscreen render target is
    // already bottom-up, so rows need reversing when it is *not* flagged
    // (verified against a real capture: unflipped output was upside down).
    let bottom_up = !mapping.flipped();
    // Abgr8888 is R, G, B, A in memory: exactly PNG's RGBA.
    let mut pixels = renderer.map_texture(&mapping).map_err(|e| e.to_string())?.to_vec();
    if bottom_up {
        flip_rows(&mut pixels, size.w as usize * 4);
    }
    Ok((pixels, size.w as u32, size.h as u32))
}

/// Called by the backend after it drew a frame with a capture pending.
pub fn capture_and_save(renderer: &mut GlesRenderer, output: &Output, state: &mut WhiteRose) {
    match capture(renderer, output, state) {
        Ok((pixels, w, h)) => {
            let tx = state.screenshot_tx.clone();
            let dir = screenshots_dir();
            std::thread::spawn(move || {
                let _ = tx.send(encode_and_save(&pixels, w, h, &dir, Local::now()));
            });
        }
        Err(e) => state.screenshot_done(Outcome::Failed(e)),
    }
}

impl WhiteRose {
    /// PrtSc: capture the whole screen on the next frame.
    pub fn request_screenshot(&mut self) {
        if self.redraw.is_none() {
            self.screenshot_done(Outcome::Failed("no renderer (headless)".into()));
            return;
        }
        self.screenshot_pending = true;
        self.schedule_redraw();
    }

    /// Main-loop side of a finished capture.
    pub fn screenshot_done(&mut self, outcome: Outcome) {
        let event = match outcome {
            Outcome::Saved { path, width, height, png } => {
                tracing::info!(path = %path.display(), width, height, "screenshot saved");
                set_data_device_selection(&self.display_handle, &self.seat, vec!["image/png".into()], png);
                self.saved_screenshots.push(path.clone());
                ipc::Event::Screenshot { path: path.to_string_lossy().into_owned(), width, height }
            }
            Outcome::Failed(error) => {
                tracing::warn!(error, "screenshot failed");
                ipc::Event::ScreenshotFailed { error }
            }
        };
        if let Some(ipc) = self.ipc.as_mut() {
            ipc.update(event);
        }
    }

    /// Deletes a screenshot, but only one this compositor saved: the shell
    /// can't use this to remove arbitrary files.
    pub fn delete_screenshot(&mut self, path: &Path) -> bool {
        let Some(i) = self.saved_screenshots.iter().position(|p| p == path) else {
            tracing::warn!(path = %path.display(), "refusing to delete a file that isn't one of our screenshots");
            return false;
        };
        let p = self.saved_screenshots.swap_remove(i);
        std::fs::remove_file(&p).map_err(|e| tracing::warn!(%e, "delete screenshot")).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn flips_rows() {
        let mut px = vec![1, 1, 2, 2, 3, 3];
        flip_rows(&mut px, 2);
        assert_eq!(px, vec![3, 3, 2, 2, 1, 1]);
        let mut even = vec![1, 2, 3, 4];
        flip_rows(&mut even, 1);
        assert_eq!(even, vec![4, 3, 2, 1]);
        flip_rows(&mut [], 4);
    }

    #[test]
    fn names_files_by_local_time() {
        let t = Local.with_ymd_and_hms(2026, 11, 9, 14, 32, 7).unwrap();
        assert_eq!(file_name(t), "Screenshot 2026-11-09 14-32-07.png");
    }

    #[test]
    fn resolves_pictures_dir() {
        let home = Path::new("/home/u");
        assert_eq!(screenshots_dir_from(home, None), PathBuf::from("/home/u/Pictures/Screenshots"));
        let dirs = "# comment\nXDG_DESKTOP_DIR=\"$HOME/Desktop\"\nXDG_PICTURES_DIR=\"$HOME/Bilder\"\n";
        assert_eq!(screenshots_dir_from(home, Some(dirs)), PathBuf::from("/home/u/Bilder/Screenshots"));
        let abs = "XDG_PICTURES_DIR=\"/data/pics\"";
        assert_eq!(screenshots_dir_from(home, Some(abs)), PathBuf::from("/data/pics/Screenshots"));
        // Relative values are invalid per the spec: fall back.
        let rel = "XDG_PICTURES_DIR=\"pics\"";
        assert_eq!(screenshots_dir_from(home, Some(rel)), PathBuf::from("/home/u/Pictures/Screenshots"));
        let commented = "#XDG_PICTURES_DIR=\"/nope\"";
        assert_eq!(screenshots_dir_from(home, Some(commented)), PathBuf::from("/home/u/Pictures/Screenshots"));
    }

    #[test]
    fn never_overwrites_an_existing_screenshot() {
        let dir = std::env::temp_dir().join(format!("wr-shot-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let name = "Screenshot 2026-11-09 14-32-07.png";
        assert_eq!(unique_path(&dir, name), dir.join(name));
        std::fs::write(dir.join(name), b"x").unwrap();
        assert_eq!(unique_path(&dir, name), dir.join("Screenshot 2026-11-09 14-32-07 (2).png"));
        std::fs::write(dir.join("Screenshot 2026-11-09 14-32-07 (2).png"), b"x").unwrap();
        assert_eq!(unique_path(&dir, name), dir.join("Screenshot 2026-11-09 14-32-07 (3).png"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn png_round_trips() {
        let (w, h) = (3u32, 2u32);
        let rgba: Vec<u8> = (0..w * h * 4).map(|i| i as u8).collect();
        let png = encode_png(&rgba, w, h).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let dec = png::Decoder::new(std::io::Cursor::new(png));
        let mut reader = dec.read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).unwrap();
        assert_eq!((info.width, info.height), (w, h));
        assert_eq!(&buf[..info.buffer_size()], &rgba[..]);
    }

    #[test]
    fn rejects_mismatched_buffers() {
        assert!(encode_png(&[0; 10], 2, 2).is_err());
    }

    #[test]
    fn saves_into_a_new_directory() {
        let dir = std::env::temp_dir().join(format!("wr-shot-save-{}", std::process::id())).join("Screenshots");
        let t = Local.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap();
        match encode_and_save(&[255; 16], 2, 2, &dir, t) {
            Outcome::Saved { path, width, height, png } => {
                assert_eq!(path, dir.join("Screenshot 2026-01-02 03-04-05.png"));
                assert_eq!((width, height), (2, 2));
                assert_eq!(std::fs::read(&path).unwrap(), *png);
            }
            Outcome::Failed(e) => panic!("{e}"),
        }
        std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }
}
