//! Clipboard history: what was copied, kept small, saved to disk.
//!
//! Layout under `$XDG_DATA_HOME/whiterose/clipboard/`:
//!   history.json   the entries (text inline, images by file name, files by path)
//!   images/        image data, one file per distinct image
//!
//! Limits keep it small: `MAX_ENTRIES` entries, text up to `MAX_TEXT` bytes,
//! images up to `MAX_IMAGE` each and `MAX_IMAGE_TOTAL` together; the oldest
//! entries go first. Copying something already in the history moves it to
//! the top instead of storing it twice. Anything a password manager marks as
//! secret is never stored. Copied *files* are stored as paths and re-checked
//! against the disk whenever the history is shown, so removed files are marked
//! and never pasted.

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

pub const MAX_ENTRIES: usize = 50;
pub const MAX_TEXT: usize = 256 * 1024;
pub const MAX_IMAGE: usize = 16 * 1024 * 1024;
pub const MAX_IMAGE_TOTAL: u64 = 64 * 1024 * 1024;

/// Offered by password managers (KeePassXC, KDE) on secrets.
const SECRET_HINT: &str = "x-kde-passwordManagerHint";

const TEXT_MIMES: &[&str] = &["text/plain;charset=utf-8", "UTF8_STRING", "text/plain", "STRING", "TEXT"];
const IMAGE_MIMES: &[&str] = &["image/png", "image/jpeg", "image/webp", "image/gif", "image/bmp"];

/// Which representation of a selection to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    Files(String),
    Image(String),
    Text(String),
}

impl Choice {
    pub fn mime(&self) -> &str {
        match self {
            Choice::Files(m) | Choice::Image(m) | Choice::Text(m) => m,
        }
    }

    /// The most bytes worth reading for this kind.
    pub fn limit(&self) -> usize {
        match self {
            Choice::Image(_) => MAX_IMAGE,
            // A path list is small; anything larger is not a file copy.
            Choice::Files(_) => 64 * 1024,
            Choice::Text(_) => MAX_TEXT,
        }
    }
}

/// Picks the best representation to store, or None for secrets and
/// selections we don't keep (e.g. only rich text with no plain fallback).
pub fn choose(mimes: &[String]) -> Option<Choice> {
    if mimes.iter().any(|m| m == SECRET_HINT) {
        return None;
    }
    let has = |m: &str| mimes.iter().any(|x| x.eq_ignore_ascii_case(m));
    for m in ["text/uri-list", "x-special/gnome-copied-files"] {
        if has(m) {
            return Some(Choice::Files(m.to_string()));
        }
    }
    if let Some(m) = IMAGE_MIMES.iter().find(|m| has(m)) {
        return Some(Choice::Image(m.to_string()));
    }
    TEXT_MIMES.iter().find(|m| has(m)).map(|m| Choice::Text(m.to_string()))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Content {
    Text {
        text: String,
    },
    /// `file` is a name inside the history's images directory.
    Image {
        file: String,
        mime: String,
        bytes: u64,
    },
    Files {
        paths: Vec<String>,
    },
}

impl Content {
    /// Identity used to de-duplicate repeated copies.
    pub fn key(&self) -> String {
        let mut h = DefaultHasher::new();
        match self {
            Content::Text { text } => ("text", text).hash(&mut h),
            Content::Image { file, .. } => ("image", file).hash(&mut h),
            Content::Files { paths } => ("files", paths).hash(&mut h),
        }
        format!("{:016x}", h.finish())
    }

    fn image_bytes(&self) -> u64 {
        match self {
            Content::Image { bytes, .. } => *bytes,
            _ => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub id: u64,
    /// Unix seconds of the most recent copy.
    pub time: i64,
    #[serde(flatten)]
    pub content: Content,
}

/// An entry as the shell sees it, with files checked against the disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryView {
    pub id: u64,
    pub time: i64,
    pub kind: String,
    /// Text: the first ~300 characters. Files: the file names.
    pub preview: String,
    /// Text: the whole text, for dragging it into another app. Left out for
    /// texts over DRAG_TEXT_MAX (those can still be pasted by clicking).
    pub text: Option<String>,
    /// Image: absolute path to the stored image.
    pub image: Option<String>,
    pub files: Vec<FileView>,
    /// False when nothing can be pasted (every copied file is gone).
    pub available: bool,
    /// Image: where a readably named copy is put for drag-and-drop (an email
    /// attachment called "Clipboard image ….png", not a hash). Created on
    /// request just before a drag starts.
    pub drag_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileView {
    pub path: String,
    pub exists: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct History {
    /// Newest first.
    pub entries: Vec<Entry>,
    pub next_id: u64,
}

impl History {
    /// Adds a copy. Returns the entry's id and the image files of entries
    /// that were evicted (for the caller to delete if nothing else uses them).
    pub fn add(&mut self, content: Content, now: i64) -> (u64, Vec<String>) {
        let key = content.key();
        if let Some(i) = self.entries.iter().position(|e| e.content.key() == key) {
            let mut e = self.entries.remove(i);
            e.time = now;
            let id = e.id;
            self.entries.insert(0, e);
            return (id, Vec::new());
        }
        self.next_id += 1;
        let id = self.next_id;
        self.entries.insert(0, Entry { id, time: now, content });
        let mut evicted = Vec::new();
        while self.entries.len() > MAX_ENTRIES || self.image_total() > MAX_IMAGE_TOTAL {
            if self.entries.len() <= 1 {
                break; // never evict the copy that was just made
            }
            if let Some(e) = self.entries.pop() {
                evicted.extend(image_file(&e.content));
            }
        }
        (id, self.unreferenced(evicted))
    }

    /// Moves an entry to the top (it was pasted back). Returns false if unknown.
    pub fn touch(&mut self, id: u64, now: i64) -> bool {
        let Some(i) = self.entries.iter().position(|e| e.id == id) else { return false };
        let mut e = self.entries.remove(i);
        e.time = now;
        self.entries.insert(0, e);
        true
    }

    /// Removes one entry; returns image files no longer referenced.
    pub fn remove(&mut self, id: u64) -> Vec<String> {
        let Some(i) = self.entries.iter().position(|e| e.id == id) else { return Vec::new() };
        let e = self.entries.remove(i);
        self.unreferenced(image_file(&e.content).into_iter().collect())
    }

    /// Removes everything; returns every image file.
    pub fn clear(&mut self) -> Vec<String> {
        let files = self.entries.iter().filter_map(|e| image_file(&e.content)).collect();
        self.entries.clear();
        files
    }

    pub fn get(&self, id: u64) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }

    fn image_total(&self) -> u64 {
        self.entries.iter().map(|e| e.content.image_bytes()).sum()
    }

    fn unreferenced(&self, files: Vec<String>) -> Vec<String> {
        files.into_iter().filter(|f| !self.entries.iter().any(|e| image_file(&e.content).as_ref() == Some(f))).collect()
    }

    /// What the shell shows. Checks every copied file against the disk.
    pub fn view(&self, images_dir: &Path, drag_dir: &Path) -> Vec<EntryView> {
        self.entries.iter().map(|e| view_entry(e, images_dir, drag_dir)).collect()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("history serializes")
    }

    /// Loads saved history. A missing or corrupt file gives an empty history
    /// (the clipboard must never stop working because of a bad file).
    pub fn from_json(text: &str) -> History {
        let mut h: History = serde_json::from_str(text).unwrap_or_default();
        h.entries.truncate(MAX_ENTRIES);
        h.next_id = h.next_id.max(h.entries.iter().map(|e| e.id).max().unwrap_or(0));
        h
    }
}

fn image_file(c: &Content) -> Option<String> {
    match c {
        Content::Image { file, .. } => Some(file.clone()),
        _ => None,
    }
}

fn view_entry(e: &Entry, images_dir: &Path, drag_dir: &Path) -> EntryView {
    match &e.content {
        Content::Text { text } => EntryView {
            id: e.id,
            time: e.time,
            kind: "text".into(),
            preview: text.chars().take(300).collect(),
            text: (text.len() <= DRAG_TEXT_MAX).then(|| text.clone()),
            image: None,
            files: Vec::new(),
            available: true,
            drag_path: None,
        },
        Content::Image { file, .. } => {
            let path = images_dir.join(file);
            EntryView {
                id: e.id,
                time: e.time,
                kind: "image".into(),
                preview: String::new(),
                text: None,
                available: path.exists(),
                image: Some(path.to_string_lossy().into_owned()),
                files: Vec::new(),
                drag_path: Some(drag_dir.join(drag_name(e.time, file)).to_string_lossy().into_owned()),
            }
        }
        Content::Files { paths } => {
            let files: Vec<FileView> =
                paths.iter().map(|p| FileView { path: p.clone(), exists: Path::new(p).exists() }).collect();
            EntryView {
                id: e.id,
                time: e.time,
                kind: "files".into(),
                preview: paths
                    .iter()
                    .map(|p| {
                        Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.clone())
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
                text: None,
                image: None,
                available: files.iter().any(|f| f.exists),
                files,
                drag_path: None,
            }
        }
    }
}

/// Longest text sent to the shell whole (so it can be dragged out). Keeps the
/// history event small: 50 entries can't add up to more than ~800 KB.
pub const DRAG_TEXT_MAX: usize = 16 * 1024;

/// "Clipboard image 2026-11-09 14-32-07.png": the name a dragged clipboard
/// image gets, from the copy's local time and the stored file's extension.
pub fn drag_name(time: i64, stored_file: &str) -> String {
    use chrono::TimeZone;
    let ext = Path::new(stored_file).extension().and_then(|e| e.to_str()).unwrap_or("png");
    let when = chrono::Local.timestamp_opt(time, 0).single().unwrap_or_else(chrono::Local::now);
    format!("Clipboard image {}.{ext}", when.format("%Y-%m-%d %H-%M-%S"))
}

/// `$XDG_RUNTIME_DIR/whiterose/drag`: cleared at logout, never on disk.
pub fn drag_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("whiterose/drag")
}

/// Keeps the `keep` newest files in `dir` (an app may still be reading a
/// recently dropped one) and deletes the rest.
pub fn prune_dir(dir: &Path, keep: usize) {
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    files.sort_by_key(|f| std::cmp::Reverse(f.0));
    for (_, p) in files.into_iter().skip(keep) {
        let _ = std::fs::remove_file(p);
    }
}

/// Decodes %XX escapes in a URI path.
pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Encodes a path for a file:// URI (keeps unreserved characters and '/').
pub fn percent_encode(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for &c in path.as_bytes() {
        if c.is_ascii_alphanumeric() || b"/-_.~".contains(&c) {
            out.push(c as char);
        } else {
            out.push_str(&format!("%{c:02X}"));
        }
    }
    out
}

/// Local paths from text/uri-list or x-special/gnome-copied-files data.
/// Non-file URIs (http://, smb://) are dropped: they can't be checked.
pub fn parse_file_list(data: &str) -> Vec<String> {
    data.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && *l != "copy" && *l != "cut")
        .filter_map(|l| l.strip_prefix("file://"))
        // file://host/path: keep only the path.
        .map(|rest| if rest.starts_with('/') { rest } else { rest.find('/').map_or("", |i| &rest[i..]) })
        .filter(|p| !p.is_empty())
        .map(percent_decode)
        .collect()
}

/// What to offer when an entry is pasted back: (mime, bytes) pairs. Files
/// that no longer exist are left out; None if nothing is left to paste.
pub fn payloads(entry: &Entry, images_dir: &Path) -> Option<Vec<(String, Vec<u8>)>> {
    match &entry.content {
        Content::Text { text } => Some(TEXT_MIMES.iter().map(|m| (m.to_string(), text.as_bytes().to_vec())).collect()),
        Content::Image { file, mime, .. } => {
            std::fs::read(images_dir.join(file)).ok().map(|bytes| vec![(mime.clone(), bytes)])
        }
        Content::Files { paths } => {
            let alive: Vec<&String> = paths.iter().filter(|p| Path::new(p).exists()).collect();
            if alive.is_empty() {
                return None;
            }
            let uris: Vec<String> = alive.iter().map(|p| format!("file://{}", percent_encode(p))).collect();
            let plain = alive.iter().map(|p| p.as_str()).collect::<Vec<_>>().join("\n");
            Some(vec![
                ("text/uri-list".into(), format!("{}\r\n", uris.join("\r\n")).into_bytes()),
                ("x-special/gnome-copied-files".into(), format!("copy\n{}", uris.join("\n")).into_bytes()),
                ("text/plain;charset=utf-8".into(), plain.clone().into_bytes()),
                ("text/plain".into(), plain.into_bytes()),
            ])
        }
    }
}

/// `$XDG_DATA_HOME/whiterose/clipboard`.
pub fn history_dir() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"));
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"))
        .join("whiterose/clipboard")
}

/// Writes atomically (temp file + rename) so a crash never leaves a torn file.
pub fn save(dir: &Path, history: &History) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join("history.json.tmp");
    std::fs::write(&tmp, history.to_json())?;
    std::fs::rename(tmp, dir.join("history.json"))
}

pub fn load(dir: &Path) -> History {
    std::fs::read_to_string(dir.join("history.json")).map(|t| History::from_json(&t)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }
    fn text(t: &str) -> Content {
        Content::Text { text: t.into() }
    }

    #[test]
    fn chooses_files_then_images_then_text() {
        assert_eq!(choose(&s(&["text/plain", "text/uri-list"])), Some(Choice::Files("text/uri-list".into())));
        assert_eq!(
            choose(&s(&["x-special/gnome-copied-files", "UTF8_STRING"])),
            Some(Choice::Files("x-special/gnome-copied-files".into()))
        );
        assert_eq!(choose(&s(&["image/png", "text/plain"])), Some(Choice::Image("image/png".into())));
        assert_eq!(
            choose(&s(&["STRING", "text/plain;charset=utf-8"])),
            Some(Choice::Text("text/plain;charset=utf-8".into()))
        );
        assert_eq!(choose(&s(&["text/html"])), None, "rich text without a plain fallback isn't kept");
        assert_eq!(choose(&[]), None);
    }

    #[test]
    fn never_stores_password_manager_secrets() {
        assert_eq!(choose(&s(&["text/plain", "x-kde-passwordManagerHint"])), None);
    }

    #[test]
    fn repeated_copies_move_to_the_top_instead_of_duplicating() {
        let mut h = History::default();
        let (a, _) = h.add(text("one"), 1);
        h.add(text("two"), 2);
        let (again, _) = h.add(text("one"), 3);
        assert_eq!(again, a, "same entry");
        assert_eq!(h.entries.len(), 2);
        assert_eq!(h.entries[0].content, text("one"));
        assert_eq!(h.entries[0].time, 3);
    }

    #[test]
    fn keeps_at_most_max_entries_dropping_the_oldest() {
        let mut h = History::default();
        for i in 0..(MAX_ENTRIES + 7) {
            h.add(text(&format!("entry {i}")), i as i64);
        }
        assert_eq!(h.entries.len(), MAX_ENTRIES);
        assert_eq!(h.entries[0].content, text(&format!("entry {}", MAX_ENTRIES + 6)), "newest first");
        assert!(!h.entries.iter().any(|e| e.content == text("entry 0")), "oldest evicted");
    }

    #[test]
    fn image_budget_evicts_old_images_and_reports_their_files() {
        let mut h = History::default();
        let img =
            |n: usize| Content::Image { file: format!("{n}.png"), mime: "image/png".into(), bytes: 20 * 1024 * 1024 };
        h.add(img(1), 1);
        h.add(text("between"), 2);
        h.add(img(2), 3);
        h.add(img(3), 4);
        let (_, evicted) = h.add(img(4), 5); // 4 x 20 MB > 64 MB
        assert_eq!(evicted, vec!["1.png".to_string()], "oldest image's file should be deleted");
        assert!(h.image_total() <= MAX_IMAGE_TOTAL);
        assert!(h.entries.iter().any(|e| e.content == text("between")), "text kept while evicting from the tail");
    }

    #[test]
    fn clear_and_remove_report_image_files() {
        let mut h = History::default();
        let (img, _) = h.add(Content::Image { file: "a.png".into(), mime: "image/png".into(), bytes: 10 }, 1);
        h.add(text("t"), 2);
        assert_eq!(h.remove(img), vec!["a.png".to_string()]);
        assert_eq!(h.remove(999), Vec::<String>::new());
        h.add(Content::Image { file: "b.png".into(), mime: "image/png".into(), bytes: 10 }, 3);
        assert_eq!(h.clear(), vec!["b.png".to_string()]);
        assert!(h.entries.is_empty());
    }

    #[test]
    fn touch_moves_a_pasted_entry_to_the_top() {
        let mut h = History::default();
        let (a, _) = h.add(text("a"), 1);
        h.add(text("b"), 2);
        assert!(h.touch(a, 5));
        assert_eq!(h.entries[0].id, a);
        assert!(!h.touch(42, 5));
    }

    #[test]
    fn parses_file_lists() {
        let uri = "# comment\r\nfile:///home/u/My%20File.txt\r\nfile:///tmp/a%26b\r\nhttps://example.com/x\r\n";
        assert_eq!(parse_file_list(uri), vec!["/home/u/My File.txt", "/tmp/a&b"]);
        let gnome = "copy\nfile:///home/u/a.png\nfile://localhost/home/u/b.png";
        assert_eq!(parse_file_list(gnome), vec!["/home/u/a.png", "/home/u/b.png"]);
        assert!(parse_file_list("").is_empty());
    }

    #[test]
    fn percent_round_trip() {
        let p = "/home/u/R&D notes/€.txt";
        assert_eq!(percent_decode(&percent_encode(p)), p);
        assert_eq!(percent_decode("bad%zzescape%"), "bad%zzescape%", "invalid escapes are kept");
    }

    #[test]
    fn files_are_checked_against_the_disk() {
        let dir = std::env::temp_dir().join(format!("wr-clip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let alive = dir.join("still here.txt");
        std::fs::write(&alive, b"x").unwrap();
        let gone = dir.join("deleted.txt");
        let mut h = History::default();
        let (id, _) =
            h.add(Content::Files { paths: vec![alive.to_string_lossy().into(), gone.to_string_lossy().into()] }, 1);
        let v = &h.view(&dir, &dir)[0];
        assert_eq!(v.kind, "files");
        assert_eq!(v.files.iter().map(|f| f.exists).collect::<Vec<_>>(), vec![true, false]);
        assert!(v.available);
        assert_eq!(v.preview, "still here.txt, deleted.txt");

        // Pasting offers only the file that still exists.
        let p = payloads(h.get(id).unwrap(), &dir).unwrap();
        let uri_list = String::from_utf8(p.iter().find(|(m, _)| m == "text/uri-list").unwrap().1.clone()).unwrap();
        assert!(uri_list.contains("still%20here.txt") && !uri_list.contains("deleted"), "{uri_list}");

        // Once every file is gone, the entry can't be pasted.
        std::fs::remove_file(&alive).unwrap();
        assert!(!h.view(&dir, &dir)[0].available);
        assert!(payloads(h.get(id).unwrap(), &dir).is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn text_payload_covers_common_mime_names() {
        let mut h = History::default();
        let (id, _) = h.add(text("héllo"), 1);
        let p = payloads(h.get(id).unwrap(), Path::new("/nonexistent")).unwrap();
        assert!(p.iter().any(|(m, b)| m == "UTF8_STRING" && b == "héllo".as_bytes()));
        assert!(p.iter().any(|(m, _)| m == "text/plain;charset=utf-8"));
    }

    #[test]
    fn saves_and_loads_and_survives_corruption() {
        let dir = std::env::temp_dir().join(format!("wr-clip-save-{}", std::process::id()));
        let mut h = History::default();
        h.add(text("persist me"), 7);
        h.add(Content::Files { paths: vec!["/a b/c".into()] }, 8);
        save(&dir, &h).unwrap();
        assert_eq!(load(&dir), h);
        assert!(!dir.join("history.json.tmp").exists(), "temp file renamed away");
        std::fs::write(dir.join("history.json"), "{ not json").unwrap();
        assert_eq!(load(&dir), History::default(), "corrupt file: start empty, don't crash");
        // ids keep increasing after a reload, so a restored history never reuses one.
        let reloaded = History::from_json(&h.to_json());
        assert_eq!(reloaded.next_id, 2);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn dragged_images_get_readable_names() {
        use chrono::TimeZone;
        let t = chrono::Local.with_ymd_and_hms(2026, 11, 9, 14, 32, 7).unwrap().timestamp();
        assert_eq!(drag_name(t, "3f9a1c2b7e440000.png"), "Clipboard image 2026-11-09 14-32-07.png");
        assert_eq!(drag_name(t, "abc.jpg"), "Clipboard image 2026-11-09 14-32-07.jpg");
        let mut h = History::default();
        h.add(Content::Image { file: "abc.png".into(), mime: "image/png".into(), bytes: 1 }, t);
        let v = &h.view(Path::new("/img"), Path::new("/run/drag"))[0];
        assert_eq!(v.drag_path.as_deref(), Some("/run/drag/Clipboard image 2026-11-09 14-32-07.png"));
    }

    #[test]
    fn prune_keeps_the_newest_files() {
        let dir = std::env::temp_dir().join(format!("wr-prune-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..5 {
            let p = dir.join(format!("{i}"));
            std::fs::write(&p, b"x").unwrap();
            let t = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000 + i);
            std::fs::File::options().write(true).open(&p).unwrap().set_modified(t).unwrap();
        }
        prune_dir(&dir, 2);
        let mut left: Vec<String> =
            std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        left.sort();
        assert_eq!(left, vec!["3", "4"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn short_texts_are_sent_whole_for_dragging() {
        let mut h = History::default();
        h.add(Content::Text { text: "x".repeat(DRAG_TEXT_MAX + 1) }, 1);
        h.add(Content::Text { text: "y".repeat(400) }, 2);
        let v = h.view(Path::new("/i"), Path::new("/d"));
        assert_eq!(v[0].text.as_deref().map(str::len), Some(400), "whole, not the 300-char preview");
        assert_eq!(v[0].preview.len(), 300);
        assert_eq!(v[1].text, None, "too long to send; click to paste instead");
    }
}
