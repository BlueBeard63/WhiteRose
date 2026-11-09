//! Compositor side of clipboard history: reads each new clipboard selection
//! from the app that owns it (on a worker thread, with a timeout and a size
//! cap), records it, and pastes entries back when the shell asks.

use std::{
    io::Read,
    os::{fd::OwnedFd, unix::net::UnixStream},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use smithay::wayland::selection::data_device::{request_data_device_client_selection, set_data_device_selection};

use crate::{
    clipboard::{self, Choice, Content},
    ipc,
    state::WhiteRose,
};

/// Result of reading one selection, sent back to the main loop.
pub struct Captured(pub Option<Content>);

/// How long an app gets to hand over its clipboard data.
const READ_TIMEOUT: Duration = Duration::from_secs(3);

/// Reads at most `limit` bytes; None if the data is larger (not stored) or unreadable.
fn read_capped(mut r: impl Read, limit: usize) -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    let n = r.by_ref().take(limit as u64 + 1).read_to_end(&mut buf).ok()?;
    (n <= limit).then_some(buf)
}

fn ext_for(mime: &str) -> &'static str {
    match mime {
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/bmp" => "bmp",
        _ => "png",
    }
}

/// Stores image bytes under their content hash (identical images share a file).
pub fn store_image(images_dir: &Path, mime: &str, bytes: &[u8]) -> Option<Content> {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut h);
    let file = format!("{:016x}.{}", h.finish(), ext_for(mime));
    std::fs::create_dir_all(images_dir).ok()?;
    let path = images_dir.join(&file);
    if !path.exists() {
        std::fs::write(&path, bytes).ok()?;
    }
    Some(Content::Image { file, mime: mime.to_string(), bytes: bytes.len() as u64 })
}

/// Turns raw selection data into history content (on the worker thread).
pub fn to_content(choice: &Choice, data: &[u8], images_dir: &Path) -> Option<Content> {
    match choice {
        Choice::Text(_) => {
            let text = String::from_utf8_lossy(data).into_owned();
            // Whitespace-only copies aren't worth a history slot.
            (!text.trim().is_empty()).then_some(Content::Text { text })
        }
        Choice::Files(_) => {
            let paths = clipboard::parse_file_list(&String::from_utf8_lossy(data));
            (!paths.is_empty()).then_some(Content::Files { paths })
        }
        Choice::Image(mime) => store_image(images_dir, mime, data),
    }
}

impl WhiteRose {
    pub fn clipboard_images_dir(&self) -> PathBuf {
        self.clipboard_dir.join("images")
    }

    /// A client set the clipboard: read and record it.
    pub fn capture_clipboard(&mut self, mimes: Vec<String>) {
        let Some(choice) = clipboard::choose(&mimes) else { return };
        let Ok((reader, writer)) = UnixStream::pair() else { return };
        if reader.set_read_timeout(Some(READ_TIMEOUT)).is_err() {
            return;
        }
        if request_data_device_client_selection(&self.seat, choice.mime().to_string(), OwnedFd::from(writer)).is_err() {
            return;
        }
        // The owning app only starts writing once it receives the request.
        let _ = self.display_handle.flush_clients();

        let tx = self.clipboard_tx.clone();
        let images = self.clipboard_images_dir();
        std::thread::spawn(move || {
            let content = read_capped(reader, choice.limit()).and_then(|d| to_content(&choice, &d, &images));
            let _ = tx.send(Captured(content));
        });
    }

    /// Main-loop side of a finished capture.
    pub fn clipboard_captured(&mut self, captured: Captured) {
        let Some(content) = captured.0 else { return };
        let (_, evicted) = self.clipboard.add(content, now());
        self.after_clipboard_change(evicted);
    }

    /// Adds a screenshot to the history (it is already on the clipboard).
    pub fn clipboard_add_screenshot(&mut self, png: Arc<Vec<u8>>) {
        let tx = self.clipboard_tx.clone();
        let images = self.clipboard_images_dir();
        std::thread::spawn(move || {
            let _ = tx.send(Captured(store_image(&images, "image/png", &png)));
        });
    }

    fn after_clipboard_change(&mut self, unused_images: Vec<String>) {
        let images = self.clipboard_images_dir();
        for f in unused_images {
            let _ = std::fs::remove_file(images.join(f));
        }
        if let Err(e) = clipboard::save(&self.clipboard_dir, &self.clipboard) {
            tracing::warn!(%e, "saving clipboard history failed");
        }
        self.clipboard_sync();
    }

    /// Sends the history to the shell, re-checking copied files on disk.
    pub fn clipboard_sync(&mut self) {
        let entries = self.clipboard.view(&self.clipboard_images_dir());
        if let Some(ipc) = self.ipc.as_mut() {
            ipc.update(ipc::Event::Clipboard { entries });
        }
    }

    /// Puts an entry back on the clipboard. Missing files are left out.
    pub fn clipboard_paste(&mut self, id: u64) {
        let Some(entry) = self.clipboard.get(id).cloned() else { return };
        let Some(payloads) = clipboard::payloads(&entry, &self.clipboard_images_dir()) else {
            tracing::info!(id, "clipboard entry can't be pasted: its files are gone");
            self.clipboard_sync();
            return;
        };
        let mimes = payloads.iter().map(|(m, _)| m.clone()).collect();
        set_data_device_selection(&self.display_handle, &self.seat, mimes, Arc::new(payloads));
        self.clipboard.touch(id, now());
        self.after_clipboard_change(Vec::new());
    }

    pub fn clipboard_delete(&mut self, id: u64) {
        let unused = self.clipboard.remove(id);
        self.after_clipboard_change(unused);
    }

    pub fn clipboard_clear(&mut self) {
        let unused = self.clipboard.clear();
        self.after_clipboard_change(unused);
    }

    /// The newest screenshots in the Screenshots folder, for the tray.
    pub fn screenshots_sync(&mut self) {
        let dir = crate::screenshot::screenshots_dir();
        let mut files: Vec<(std::time::SystemTime, String)> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("png")))
            .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path().to_string_lossy().into_owned())))
            .collect();
        files.sort_by_key(|f| std::cmp::Reverse(f.0));
        let files = files.into_iter().take(24).map(|(_, p)| p).collect();
        if let Some(ipc) = self.ipc.as_mut() {
            ipc.update(ipc::Event::Screenshots { files });
        }
    }

    /// Copies a screenshot from the tray back onto the clipboard.
    pub fn screenshot_copy(&mut self, path: &Path) {
        if !self.is_screenshot_file(path) {
            tracing::warn!(path = %path.display(), "refusing to copy a file outside the Screenshots folder");
            return;
        }
        let Ok(png) = std::fs::read(path) else { return };
        let png = Arc::new(png);
        set_data_device_selection(
            &self.display_handle,
            &self.seat,
            vec!["image/png".into()],
            Arc::new(vec![("image/png".to_string(), png.as_ref().clone())]),
        );
        self.clipboard_add_screenshot(png);
    }

    /// A PNG directly inside the Screenshots folder (no `..` tricks).
    pub fn is_screenshot_file(&self, path: &Path) -> bool {
        let dir = crate::screenshot::screenshots_dir();
        let (Ok(dir), Ok(p)) = (dir.canonicalize(), path.canonicalize()) else { return false };
        p.parent() == Some(dir.as_path()) && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("png"))
    }
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_within_the_cap_and_rejects_larger() {
        assert_eq!(read_capped(&b"hello"[..], 5), Some(b"hello".to_vec()));
        assert_eq!(read_capped(&b"hello!"[..], 5), None, "over the cap: not stored at all");
    }

    #[test]
    fn whitespace_and_empty_file_lists_are_not_stored() {
        let d = Path::new("/nonexistent");
        assert_eq!(to_content(&Choice::Text("text/plain".into()), b"  \n\t", d), None);
        assert_eq!(to_content(&Choice::Files("text/uri-list".into()), b"https://x.y/z\n", d), None);
        assert_eq!(
            to_content(&Choice::Text("text/plain".into()), b"keep me", d),
            Some(Content::Text { text: "keep me".into() })
        );
    }

    #[test]
    fn identical_images_share_one_file() {
        let dir = std::env::temp_dir().join(format!("wr-clipimg-{}", std::process::id()));
        let a = store_image(&dir, "image/png", b"\x89PNG fake").unwrap();
        let b = store_image(&dir, "image/png", b"\x89PNG fake").unwrap();
        assert_eq!(a, b);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
