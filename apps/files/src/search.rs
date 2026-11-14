//! Finding files by name: what's asked (words, `*.pdf` / `ext:pdf`, and
//! type / size / date filters), how a name matches and ranks, a live walk
//! of one folder's tree, and the index for searching home and the drives
//! at once.
//!
//! The index is one file (`$XDG_CACHE_HOME/whiterose/files/index`, only
//! yours), written beside and renamed into place. Searching maps it in and
//! reads it straight through, so it costs no memory to keep. Folders come
//! before what's in them, each tree (home, each drive) in one stretch:
//!
//! ```text
//! "WRIX1\0" | trees: u32 | per tree: start u32, end u32, path (u16 + bytes)
//! records: parent u32 (folder number; MAX for a tree's top), flags u8
//!          (1 folder, 2 link), size u64, modified i64 (s), name (u16 + bytes)
//! folder table: one u64 offset per folder, in folder order | its offset u64
//! ```

use std::{
    collections::VecDeque,
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

use serde::{Deserialize, Serialize};

// ---- the question ----------------------------------------------------------------

#[derive(Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct Ask {
    pub text: String,
    /// here | home | everywhere
    pub scope: String,
    /// The folder for "here".
    pub here: String,
    /// "" (any), folder, document, image, audio, video, archive, code
    pub kind: String,
    /// "" (any), small (< 1 MB), medium (1–100 MB), large (100 MB–1 GB), huge (> 1 GB)
    pub size: String,
    /// "" (any), today, week, month, year
    pub modified: String,
    pub hidden: bool,
    /// Letter case must match.
    pub case: bool,
    /// The text (less tag: and ext: parts) is a regular expression.
    pub regex: bool,
    /// Text the file must have inside it (its contents, a PDF's text, or
    /// its comment); "" for names only. Same case and regex rules.
    pub content: String,
    /// With content: a file is found by its name *or* what's inside it
    /// (the search box's words looked for in both), not both at once.
    pub either: bool,
}

// ---- inside files ---------------------------------------------------------------------

/// What must be inside a file, and how it's looked for.
pub struct Inside {
    re: regex::Regex,
    /// pdftotext is installed: PDFs' text is looked in too.
    pub pdf: bool,
}

/// Files bigger than this aren't read through.
pub const INSIDE_MAX: u64 = 20 << 20;

impl Inside {
    pub fn new(text: &str, case: bool, regex: bool) -> Result<Inside, String> {
        let pat = if regex {
            text.to_string()
        } else {
            regex::escape(text.trim())
        };
        let re = regex::RegexBuilder::new(&pat)
            .case_insensitive(!case)
            .size_limit(1 << 20)
            .build()
            .map_err(|e| e.to_string().lines().last().unwrap_or("").to_string())?;
        Ok(Inside {
            re,
            pdf: crate::launch::installed("pdftotext"),
        })
    }

    fn line_of(&self, text: &str) -> Option<String> {
        let m = self.re.find(text)?;
        let line_no = text[..m.start()].matches('\n').count() + 1;
        let start = text[..m.start()].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let end = text[m.end()..]
            .find('\n')
            .map(|i| m.end() + i)
            .unwrap_or(text.len());
        let line = text[start..end].trim();
        // Around the match, at most ~100 characters.
        let line: String = if line.chars().count() > 100 {
            let at = line.find(m.as_str()).unwrap_or(0);
            let from = line[..at]
                .char_indices()
                .rev()
                .nth(30)
                .map(|(i, _)| i)
                .unwrap_or(0);
            let s: String = line[from..].chars().take(100).collect();
            format!("{}{s}…", if from > 0 { "…" } else { "" })
        } else {
            line.to_string()
        };
        Some(format!("{line_no}: {line}"))
    }

    /// Where in `path` it is ("12: the line"), or None. Text files (no NUL
    /// near the start), PDFs (with pdftotext), and the file's comment.
    pub fn find(&self, path: &Path, size: u64, mime: &str) -> Option<String> {
        let comment = crate::tags::comment(path);
        if !comment.is_empty() && self.re.is_match(&comment) {
            return Some(format!("comment: {comment}"));
        }
        if size == 0 || size > INSIDE_MAX {
            return None;
        }
        if mime == "application/pdf" {
            if !self.pdf {
                return None;
            }
            let out = std::process::Command::new("pdftotext")
                .args(["-l", "200", "-q"])
                .arg(path)
                .arg("-")
                .stdin(std::process::Stdio::null())
                .output()
                .ok()?;
            let text = String::from_utf8_lossy(&out.stdout);
            return self.find_text(&text).map(|l| format!("page text {l}"));
        }
        let bytes = fs::read(path).ok()?;
        if bytes[..bytes.len().min(8192)].contains(&0) {
            return None;
        }
        self.find_text(&String::from_utf8_lossy(&bytes))
    }

    fn find_text(&self, text: &str) -> Option<String> {
        self.line_of(text).map(|l| format!("line {l}"))
    }
}

/// What a name must have: words (lowercase unless matching case, any
/// order) and extensions; or a regular expression.
#[derive(Debug, Default)]
pub struct Query {
    pub words: Vec<String>,
    pub exts: Vec<String>,
    /// tag:work: the file must have these tags (as written, any case).
    pub tags: Vec<String>,
    pub case: bool,
    pub pattern: Option<regex::Regex>,
    /// Why the regular expression can't be used ("" if it can).
    pub bad: String,
}

impl PartialEq for Query {
    fn eq(&self, o: &Query) -> bool {
        self.words == o.words && self.exts == o.exts && self.tags == o.tags && self.case == o.case
    }
}

impl Query {
    /// The file at `p` has every tag asked for.
    pub fn tagged(&self, p: &Path) -> bool {
        if self.tags.is_empty() {
            return true;
        }
        let on: Vec<String> = crate::tags::read(p)
            .iter()
            .map(|t| t.to_lowercase())
            .collect();
        self.tags.iter().all(|t| on.contains(t))
    }
}

#[cfg(test)]
pub fn parse(text: &str) -> Query {
    parse_with(text, false, false)
}

/// The question as asked: with `case`, letters must match exactly; with
/// `regex`, what's left after tag: and ext: is one regular expression.
pub fn parse_with(text: &str, case: bool, regex: bool) -> Query {
    let mut q = Query {
        case,
        ..Query::default()
    };
    let mut rest = vec![];
    for w in text.split_whitespace() {
        let lw = w.to_lowercase();
        if let Some(t) = lw.strip_prefix("tag:") {
            q.tags
                .extend(t.split(',').filter(|t| !t.is_empty()).map(str::to_string));
        } else if let Some(e) = lw.strip_prefix("ext:").or_else(|| lw.strip_prefix("*.")) {
            q.exts.extend(
                e.split(',')
                    .filter(|e| !e.is_empty())
                    .map(|e| e.trim_start_matches('.').to_string()),
            );
        } else {
            rest.push(w);
            q.words.push(if case { w.to_string() } else { lw });
        }
    }
    if regex && !rest.is_empty() {
        q.words.clear();
        match regex::RegexBuilder::new(&rest.join(" "))
            .case_insensitive(!case)
            .size_limit(1 << 20)
            .build()
        {
            Ok(r) => q.pattern = Some(r),
            Err(e) => {
                q.bad = e
                    .to_string()
                    .lines()
                    .last()
                    .unwrap_or("not a valid pattern")
                    .trim()
                    .trim_start_matches("error: ")
                    .to_string()
            }
        }
    }
    q
}

/// How well `name` matches (None: it doesn't). Every word must be in it;
/// a name starting with the first word, words at word starts, and shorter
/// names rank higher.
pub fn score(name: &str, q: &Query) -> Option<i32> {
    if !q.bad.is_empty() {
        return None;
    }
    let lower = name.to_lowercase();
    if !q.exts.is_empty() {
        let ext = lower.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
        if !q.exts.iter().any(|e| e == ext) {
            return None;
        }
    }
    if let Some(re) = &q.pattern {
        let m = re.find(name)?;
        // Matching from the start, and more of the name, ranks higher.
        let s = if m.start() == 0 { 100 } else { 0 } + (m.len() * 40 / name.len().max(1)) as i32;
        return Some(s - (name.len() as i32).min(60) / 3);
    }
    let lower = if q.case { name.to_string() } else { lower };
    let mut s = 0;
    for (i, w) in q.words.iter().enumerate() {
        let at = lower.find(w.as_str())?;
        if at == 0 {
            s += if i == 0 { 100 } else { 30 };
        } else if !lower.as_bytes()[at - 1].is_ascii_alphanumeric() {
            s += 25;
        }
        if lower == *w {
            s += 50;
        }
    }
    Some(s - (lower.len() as i32).min(60) / 3)
}

/// A file's sort of thing, from its type.
pub fn kind_of(mime: &str, dir: bool) -> &'static str {
    if dir {
        return "folder";
    }
    let (media, sub) = mime.split_once('/').unwrap_or((mime, ""));
    match media {
        "image" => return "image",
        "audio" => return "audio",
        "video" => return "video",
        _ => {}
    }
    if [
        "zip",
        "x-tar",
        "x-compressed-tar",
        "x-7z-compressed",
        "x-rar",
        "vnd.rar",
        "x-xz",
        "gzip",
        "x-bzip2",
        "zstd",
        "x-xz-compressed-tar",
        "x-bzip-compressed-tar",
        "x-zstd-compressed-tar",
    ]
    .contains(&sub)
    {
        return "archive";
    }
    if sub == "pdf"
        || sub.contains("opendocument")
        || sub.contains("officedocument")
        || sub.contains("msword")
        || sub.contains("ms-excel")
        || sub.contains("ms-powerpoint")
        || mime == "text/plain"
        || mime == "text/markdown"
        || sub == "rtf"
        || sub == "epub+zip"
    {
        return "document";
    }
    if media == "text"
        || [
            "json",
            "x-shellscript",
            "javascript",
            "x-python",
            "toml",
            "x-yaml",
            "xml",
            "sql",
            "x-perl",
            "x-ruby",
            "x-php",
        ]
        .contains(&sub)
    {
        return "code";
    }
    "other"
}

/// The filters other than the name, checked once the name matches.
pub struct Filters {
    pub kind: String,
    pub size: (u64, u64),
    pub since: i64,
}

impl Filters {
    pub fn from(a: &Ask, now: i64) -> Filters {
        let size = match a.size.as_str() {
            "small" => (0, 1_000_000),
            "medium" => (1_000_000, 100_000_000),
            "large" => (100_000_000, 1_000_000_000),
            "huge" => (1_000_000_000, u64::MAX),
            _ => (0, u64::MAX),
        };
        let day = 86_400;
        let since = match a.modified.as_str() {
            "today" => now - now.rem_euclid(day) - local_offset(),
            "week" => now - 7 * day,
            "month" => now - 30 * day,
            "year" => now - 365 * day,
            _ => i64::MIN,
        };
        Filters {
            kind: a.kind.clone(),
            size,
            since,
        }
    }

    pub fn pass(&self, mime: impl FnOnce() -> String, dir: bool, size: u64, modified: i64) -> bool {
        if modified < self.since {
            return false;
        }
        // Folders have no size of their own: a size filter means files.
        if (self.size != (0, u64::MAX)) && (dir || size < self.size.0 || size >= self.size.1) {
            return false;
        }
        self.kind.is_empty() || kind_of(&mime(), dir) == self.kind
    }
}

/// Seconds east of UTC here (so "today" starts at local midnight).
fn local_offset() -> i64 {
    // SAFETY: localtime_r fills `tm` from a valid time_t.
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        tm.tm_gmtoff
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Hit {
    pub path: String,
    pub name: String,
    pub dir: bool,
    pub link: bool,
    pub size: u64,
    /// ms, like listings.
    pub modified: i64,
    #[serde(skip)]
    pub score: i32,
    /// Searching inside: where it was found ("line 12: …").
    #[serde(skip_serializing_if = "String::is_empty")]
    pub found: String,
}

/// Best first: score, then newest.
pub fn rank(hits: &mut Vec<Hit>, keep: usize) {
    hits.sort_by(|a, b| b.score.cmp(&a.score).then(b.modified.cmp(&a.modified)));
    hits.truncate(keep);
}

/// Folders not worth looking in: hidden ones, and dependency and cache trees.
pub fn skip_dir(name: &str, hidden: bool) -> bool {
    (!hidden && name.starts_with('.'))
        || ["node_modules", "__pycache__", "site-packages"].contains(&name)
}

/// Trees under an indexed folder left out of the index: Go's module and
/// build caches (thousands of other people's files).
const SKIP_UNDER_ROOT: &[&str] = &["go/pkg"];

// ---- searching a folder's tree as it is ------------------------------------------------

/// Walks `root` breadth first; `found` gets the hits so far now and then
/// (at most `keep`), the last time with done = true.
/// What a walk looks for: the name, the filters, hidden files or not, and
/// how to tell a name's type.
pub struct Look<'a> {
    pub query: &'a Query,
    pub filters: &'a Filters,
    pub hidden: bool,
    pub mime: &'a dyn Fn(&str) -> String,
    /// Searching inside files too.
    pub inside: Option<&'a Inside>,
    /// With `inside`: the name or the inside will do (else both must match).
    pub either: bool,
}

pub fn walk(
    root: &Path,
    look: &Look,
    stop: &AtomicBool,
    keep: usize,
    found: &mut dyn FnMut(&[Hit], u64, bool),
) {
    let (q, f, hidden, mime) = (look.query, look.filters, look.hidden, look.mime);
    let mut queue = VecDeque::from([root.to_path_buf()]);
    let mut hits: Vec<Hit> = vec![];
    let mut seen = 0u64;
    let mut last = std::time::Instant::now();
    while let Some(dir) = queue.pop_front() {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            seen += 1;
            let name = e.file_name().to_string_lossy().into_owned();
            if !hidden && name.starts_with('.') {
                continue;
            }
            let Ok(m) = e.metadata() else { continue };
            let is_dir = m.is_dir();
            if is_dir && !skip_dir(&name, hidden) {
                queue.push_back(e.path());
            }
            let named = score(&name, q);
            let either = look.either && look.inside.is_some();
            if named.is_none() && !either {
                continue;
            }
            if !f.pass(|| mime(&name), is_dir, m.len(), m.mtime()) || !q.tagged(&e.path()) {
                continue;
            }
            let found = match look.inside {
                Some(inside) => {
                    let inside = if is_dir || !m.is_file() {
                        None
                    } else {
                        inside.find(&e.path(), m.len(), &mime(&name))
                    };
                    match (inside, named.is_some() && either) {
                        (Some(f), _) => f,
                        // Either will do: found by its name alone.
                        (None, true) => String::new(),
                        (None, false) => continue,
                    }
                }
                None => String::new(),
            };
            // (Found only inside: after the names that match.)
            let s = named.unwrap_or(0);
            hits.push(Hit {
                found,
                path: e.path().to_string_lossy().into_owned(),
                name,
                dir: is_dir,
                link: m.file_type().is_symlink(),
                size: if is_dir { 0 } else { m.len() },
                modified: m.mtime() * 1000,
                score: s,
            });
            if hits.len() > keep * 2 {
                rank(&mut hits, keep);
            }
        }
        if last.elapsed().as_millis() >= 150 {
            last = std::time::Instant::now();
            rank(&mut hits, keep);
            found(&hits, seen, false);
        }
    }
    rank(&mut hits, keep);
    found(&hits, seen, true);
}

// ---- the index --------------------------------------------------------------------------

const MAGIC: &[u8; 6] = b"WRIX1\0";
/// Entries indexed at most.
const MAX_ENTRIES: usize = 3_000_000;

pub fn index_file() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| crate::places::home().join(".cache"))
        .join("whiterose/files/index")
}

fn put_name(out: &mut Vec<u8>, name: &[u8]) {
    let n = name.len().min(u16::MAX as usize);
    out.extend_from_slice(&(n as u16).to_le_bytes());
    out.extend_from_slice(&name[..n]);
}

/// Writes the index of `roots` to `file` (hidden folders and those
/// skip_dir names left out); returns how many entries.
#[cfg(test)]
pub fn build(roots: &[PathBuf], file: &Path, stop: &AtomicBool) -> Result<usize, String> {
    build_leaving(roots, &[], file, stop)
}

/// As build, leaving out the folders in `exclude` (and what's in them).
#[cfg(test)]
pub fn build_leaving(
    roots: &[PathBuf],
    exclude: &[PathBuf],
    file: &Path,
    stop: &AtomicBool,
) -> Result<usize, String> {
    build_tagged(roots, exclude, file, stop, &mut vec![], &mut |_| {})
}

/// As build_leaving, also gathering the tags found on files ([path, tags]),
/// and telling `progress` how many entries so far (every 1024).
pub fn build_tagged(
    roots: &[PathBuf],
    exclude: &[PathBuf],
    file: &Path,
    stop: &AtomicBool,
    tagged: &mut Vec<(String, Vec<String>)>,
    progress: &mut dyn FnMut(usize),
) -> Result<usize, String> {
    use std::os::unix::ffi::OsStrExt;
    let mut head = MAGIC.to_vec();
    head.extend_from_slice(&(roots.len() as u32).to_le_bytes());
    let mut body: Vec<u8> = vec![];
    let mut folders: Vec<u64> = vec![];
    let mut ranges = vec![];
    let mut count = 0usize;
    for root in roots {
        let start = count as u32;
        // (folder number, path) to look in.
        let mut queue: VecDeque<(u32, PathBuf)> = VecDeque::new();
        // The tree's top is a folder record named by its whole path.
        folders.push(body.len() as u64);
        body.extend_from_slice(&u32::MAX.to_le_bytes());
        body.push(1);
        body.extend_from_slice(&0u64.to_le_bytes());
        body.extend_from_slice(&0i64.to_le_bytes());
        put_name(&mut body, root.as_os_str().as_bytes());
        count += 1;
        queue.push_back(((folders.len() - 1) as u32, root.clone()));
        while let Some((num, dir)) = queue.pop_front() {
            if stop.load(Ordering::Relaxed) {
                return Err("stopped".into());
            }
            let Ok(rd) = fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                if count >= MAX_ENTRIES {
                    break;
                }
                let name = e.file_name();
                let name_s = name.to_string_lossy();
                if name_s.starts_with('.') {
                    continue;
                }
                let Ok(m) = e.metadata() else { continue };
                let is_dir = m.is_dir();
                if is_dir
                    && (skip_dir(&name_s, false)
                        || SKIP_UNDER_ROOT.iter().any(|s| e.path() == root.join(s))
                        || exclude.iter().any(|x| e.path() == *x))
                {
                    continue;
                }
                if is_dir {
                    folders.push(body.len() as u64);
                    queue.push_back(((folders.len() - 1) as u32, e.path()));
                }
                let t = crate::tags::read(&e.path());
                if !t.is_empty() {
                    tagged.push((e.path().to_string_lossy().into_owned(), t));
                }
                body.extend_from_slice(&num.to_le_bytes());
                body.push(u8::from(is_dir) | if m.file_type().is_symlink() { 2 } else { 0 });
                body.extend_from_slice(&(if is_dir { 0 } else { m.len() }).to_le_bytes());
                body.extend_from_slice(&m.mtime().to_le_bytes());
                put_name(&mut body, name.as_bytes());
                count += 1;
                if count.is_multiple_of(1024) {
                    progress(count);
                }
            }
        }
        ranges.push((start, count as u32, root.clone()));
    }
    for (start, end, root) in &ranges {
        head.extend_from_slice(&start.to_le_bytes());
        head.extend_from_slice(&end.to_le_bytes());
        put_name(&mut head, root.as_os_str().as_bytes());
    }
    // Offsets in the folder table count from the start of the file.
    let base = head.len() as u64;
    let table_at = base + body.len() as u64;
    let dir = file.parent().ok_or("no folder")?;
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut all = head;
    all.reserve(body.len() + folders.len() * 8 + 8);
    all.extend_from_slice(&body);
    drop(body);
    for f in folders {
        all.extend_from_slice(&(base + f).to_le_bytes());
    }
    all.extend_from_slice(&table_at.to_le_bytes());
    // Only yours; sealed when Settings › Files › Privacy says (secure.rs).
    crate::secure::write(file, &all)?;
    Ok(count)
}

/// One tree in the index: its records' numbers [start, end) and its top.
type Tree = (u32, u32, PathBuf);

/// The index file, mapped in (or, sealed, opened into memory).
pub struct Index {
    ptr: *mut libc::c_void,
    len: usize,
    owned: Option<Vec<u8>>,
}

// SAFETY: a read-only private mapping; only read.
unsafe impl Send for Index {}

impl Drop for Index {
    fn drop(&mut self) {
        if self.owned.is_none() {
            // SAFETY: unmapping what open() mapped.
            unsafe {
                libc::munmap(self.ptr, self.len);
            }
        }
    }
}

struct Rec<'a> {
    parent: u32,
    flags: u8,
    size: u64,
    modified: i64,
    name: &'a [u8],
    next: usize,
}

impl Index {
    pub fn open(file: &Path) -> Option<Index> {
        use std::os::unix::io::AsRawFd;
        let f = fs::File::open(file).ok()?;
        let len = f.metadata().ok()?.len() as usize;
        if len < MAGIC.len() + 4 + 8 {
            return None;
        }
        // Sealed: opened into memory.
        let mut head = [0u8; 8];
        if std::io::Read::read_exact(&mut &f, &mut head).is_ok() && crate::secure::is_sealed(&head)
        {
            let plain = crate::secure::read(file)?;
            if plain.len() < MAGIC.len() + 4 + 8 || &plain[..6] != MAGIC {
                return None;
            }
            return Some(Index {
                ptr: std::ptr::null_mut(),
                len: plain.len(),
                owned: Some(plain),
            });
        }
        // SAFETY: mapping a file we opened, read-only and private.
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ,
                libc::MAP_PRIVATE,
                f.as_raw_fd(),
                0,
            )
        };
        if ptr == libc::MAP_FAILED {
            return None;
        }
        let ix = Index {
            ptr,
            len,
            owned: None,
        };
        (&ix.bytes()[..6] == MAGIC).then_some(ix)
    }

    fn bytes(&self) -> &[u8] {
        if let Some(o) = &self.owned {
            return o;
        }
        // SAFETY: the mapping is `len` bytes and lives as long as self.
        unsafe { std::slice::from_raw_parts(self.ptr as *const u8, self.len) }
    }

    fn u16_at(&self, at: usize) -> Option<u16> {
        Some(u16::from_le_bytes(
            self.bytes().get(at..at + 2)?.try_into().ok()?,
        ))
    }
    fn u32_at(&self, at: usize) -> Option<u32> {
        Some(u32::from_le_bytes(
            self.bytes().get(at..at + 4)?.try_into().ok()?,
        ))
    }
    fn u64_at(&self, at: usize) -> Option<u64> {
        Some(u64::from_le_bytes(
            self.bytes().get(at..at + 8)?.try_into().ok()?,
        ))
    }

    fn name_at(&self, at: usize) -> Option<(&[u8], usize)> {
        let n = self.u16_at(at)? as usize;
        Some((self.bytes().get(at + 2..at + 2 + n)?, at + 2 + n))
    }

    fn rec(&self, at: usize) -> Option<Rec<'_>> {
        let parent = self.u32_at(at)?;
        let flags = *self.bytes().get(at + 4)?;
        let size = self.u64_at(at + 5)?;
        let modified = self.u64_at(at + 13)? as i64;
        let (name, next) = self.name_at(at + 21)?;
        Some(Rec {
            parent,
            flags,
            size,
            modified,
            name,
            next,
        })
    }

    /// Each tree, and where the records begin.
    fn trees(&self) -> Option<(Vec<Tree>, usize)> {
        use std::os::unix::ffi::OsStrExt;
        let n = self.u32_at(6)?;
        let mut at = 10;
        let mut v = vec![];
        for _ in 0..n {
            let s = self.u32_at(at)?;
            let e = self.u32_at(at + 4)?;
            let (p, next) = self.name_at(at + 8)?;
            v.push((s, e, PathBuf::from(std::ffi::OsStr::from_bytes(p))));
            at = next;
        }
        Some((v, at))
    }

    fn table(&self) -> Option<usize> {
        self.u64_at(self.len - 8).map(|t| t as usize)
    }

    /// A folder's whole path, from its number.
    fn folder_path(&self, num: u32) -> Option<PathBuf> {
        use std::os::unix::ffi::OsStrExt;
        let table = self.table()?;
        let mut parts: Vec<&[u8]> = vec![];
        let mut n = num;
        for _ in 0..4096 {
            let at = self.u64_at(table + n as usize * 8)? as usize;
            let r = self.rec(at)?;
            parts.push(r.name);
            if r.parent == u32::MAX {
                let mut p = PathBuf::new();
                for part in parts.iter().rev() {
                    p.push(std::ffi::OsStr::from_bytes(part));
                }
                return Some(p);
            }
            n = r.parent;
        }
        None
    }

    /// How many entries it holds (files and folders, the tops too).
    pub fn entries(&self) -> usize {
        self.trees()
            .and_then(|(t, _)| t.last().map(|x| x.1 as usize))
            .unwrap_or(0)
    }

    /// The tops of the trees in it.
    #[cfg(test)]
    pub fn roots(&self) -> Vec<PathBuf> {
        self.trees()
            .map(|(t, _)| t.into_iter().map(|x| x.2).collect())
            .unwrap_or_default()
    }

    /// Searches the trees `want` says yes to.
    pub fn find(
        &self,
        q: &Query,
        f: &Filters,
        want: &dyn Fn(&Path) -> bool,
        mime: &dyn Fn(&str) -> String,
        stop: &AtomicBool,
        keep: usize,
    ) -> (Vec<Hit>, u64) {
        let Some((trees, mut at)) = self.trees() else {
            return (vec![], 0);
        };
        let table = self.table().unwrap_or(0);
        // (the folder it's in, the hit without its path yet)
        let mut hits: Vec<(u32, Hit)> = vec![];
        let mut i = 0u32;
        let mut seen = 0u64;
        for (start, end, top) in &trees {
            let on = want(top);
            while i < *end && at < table {
                let Some(r) = self.rec(at) else { break };
                at = r.next;
                i += 1;
                if !on || i - 1 == *start {
                    continue;
                }
                seen += 1;
                if seen.is_multiple_of(65_536) && stop.load(Ordering::Relaxed) {
                    return (vec![], seen);
                }
                let name = String::from_utf8_lossy(r.name);
                let Some(s) = score(&name, q) else { continue };
                let dir = r.flags & 1 != 0;
                if !f.pass(|| mime(&name), dir, r.size, r.modified) {
                    continue;
                }
                hits.push((
                    r.parent,
                    Hit {
                        path: String::new(),
                        name: name.into_owned(),
                        dir,
                        link: r.flags & 2 != 0,
                        size: r.size,
                        modified: r.modified * 1000,
                        score: s,
                        found: String::new(),
                    },
                ));
                if hits.len() > keep * 4 {
                    best_first(&mut hits, keep);
                }
            }
        }
        best_first(&mut hits, keep);
        // Paths only for what's kept; things gone since the index was made are left out.
        let out = hits
            .into_iter()
            .filter_map(|(parent, mut hit)| {
                let path = self.folder_path(parent)?.join(&hit.name);
                fs::symlink_metadata(&path).ok()?;
                hit.path = path.to_string_lossy().into_owned();
                Some(hit)
            })
            .collect();
        (out, seen)
    }
}

fn best_first(hits: &mut Vec<(u32, Hit)>, keep: usize) {
    hits.sort_by(|a, b| {
        b.1.score
            .cmp(&a.1.score)
            .then(b.1.modified.cmp(&a.1.modified))
    });
    hits.truncate(keep);
}

/// When the index was made (unix s; 0: there's none).
pub fn index_age(file: &Path) -> i64 {
    fs::metadata(file).map(|m| m.mtime()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    fn scratch(name: &str) -> Scratch {
        let d = Scratch::new(&format!("search-{name}"));
        d
    }

    fn tree(d: &Path) {
        for p in [
            "Work/Reports/2024",
            "Work/node_modules/lib",
            "Photos/Holiday",
            ".secret",
            "go/pkg/mod/x",
        ] {
            fs::create_dir_all(d.join(p)).unwrap();
        }
        fs::write(
            d.join("Work/Reports/2024/budget report.pdf"),
            vec![0u8; 2_000_000],
        )
        .unwrap();
        fs::write(d.join("Work/Reports/notes.txt"), "x").unwrap();
        fs::write(d.join("Work/node_modules/lib/report.js"), "x").unwrap();
        fs::write(d.join("Photos/Holiday/beach.jpg"), "x").unwrap();
        fs::write(d.join(".secret/report.txt"), "x").unwrap();
        fs::write(d.join("go/pkg/mod/x/report.go"), "x").unwrap();
    }

    fn mime(n: &str) -> String {
        match n.rsplit_once('.').map(|x| x.1) {
            Some("pdf") => "application/pdf",
            Some("jpg") => "image/jpeg",
            Some("txt") => "text/plain",
            Some("js") => "application/javascript",
            _ => "application/octet-stream",
        }
        .into()
    }

    fn any() -> Filters {
        Filters::from(&Ask::default(), 0)
    }

    #[test]
    fn queries() {
        assert_eq!(
            parse("Budget  Report *.pdf"),
            Query {
                words: vec!["budget".into(), "report".into()],
                exts: vec!["pdf".into()],
                tags: vec![],
                ..Query::default()
            }
        );
        assert_eq!(parse("ext:jpg,png").exts, vec!["jpg", "png"]);
        assert_eq!(parse("tag:Work,urgent notes").tags, vec!["work", "urgent"]);
        // Matching case.
        let q = parse_with("Notes", true, false);
        assert!(score("Notes.md", &q).is_some() && score("notes.md", &q).is_none());
        // A regular expression, any case unless asked.
        let q = parse_with(r"^IMG_\d{4}\.jpe?g$ ext:jpg", false, true);
        assert!(score("img_2024.jpg", &q).is_some() && score("IMG_24.jpg", &q).is_none());
        let q = parse_with(r"^IMG", true, true);
        assert!(score("img_1.jpg", &q).is_none() && score("IMG_1.jpg", &q).is_some());
        let q = parse_with("(unclosed", false, true);
        assert!(!q.bad.is_empty() && score("unclosed", &q).is_none());
        assert_eq!(
            parse("tag:Work notes").words,
            vec!["notes"],
            "tags aren't name words"
        );
        let q = parse("report");
        assert!(
            score("Report.pdf", &q) > score("my report.pdf", &q),
            "a name starting with it first"
        );
        assert!(
            score("my report.pdf", &q) > score("myreport.pdf", &q),
            "then at a word start"
        );
        assert_eq!(score("summary.pdf", &q), None);
        let q = parse("report 2024");
        assert!(score("2024 report.odt", &q).is_some(), "any order");
        assert_eq!(score("report.odt", &q), None, "every word");
        assert_eq!(score("report.odt", &parse("*.pdf")), None);
        assert!(
            score("REPORT.PDF", &parse("*.pdf report")).is_some(),
            "case doesn't matter"
        );
    }

    #[test]
    fn kinds_and_filters() {
        assert_eq!(kind_of("application/pdf", false), "document");
        assert_eq!(kind_of("image/png", false), "image");
        assert_eq!(kind_of("application/x-compressed-tar", false), "archive");
        assert_eq!(kind_of("text/x-rust", false), "code");
        assert_eq!(kind_of("anything", true), "folder");
        let now = 1_000_000_000;
        let a = Ask {
            kind: "document".into(),
            size: "medium".into(),
            modified: "week".into(),
            ..Ask::default()
        };
        let f = Filters::from(&a, now);
        assert!(f.pass(|| "application/pdf".into(), false, 2_000_000, now - 3600));
        assert!(
            !f.pass(
                || "application/pdf".into(),
                false,
                2_000_000,
                now - 30 * 86_400
            ),
            "too old"
        );
        assert!(
            !f.pass(|| "application/pdf".into(), false, 10, now),
            "too small"
        );
        assert!(
            !f.pass(|| "image/png".into(), false, 2_000_000, now),
            "wrong kind"
        );
        assert!(
            !f.pass(|| "inode/directory".into(), true, 0, now),
            "a size means files"
        );
    }

    #[test]
    fn walking_a_tree() {
        let d = scratch("walk");
        tree(&d);
        let stop = AtomicBool::new(false);
        let mut last = vec![];
        let mut done = false;
        let (q, f) = (parse("report"), any());
        walk(
            &d,
            &Look {
                query: &q,
                filters: &f,
                hidden: false,
                mime: &mime,
                inside: None,
                either: false,
            },
            &stop,
            100,
            &mut |h, _, fin| {
                last = h.to_vec();
                done = fin;
            },
        );
        assert!(done);
        let names: Vec<&str> = last.iter().map(|h| h.name.as_str()).collect();
        // Not in node_modules or hidden folders; Go's cache is only left out of the index.
        assert_eq!(names, vec!["Reports", "report.go", "budget report.pdf"]);
        assert!(last[0].dir);
    }

    #[test]
    fn index_finds_what_a_walk_does() {
        let d = scratch("index");
        let home = d.join("home");
        let usb = d.join("usb");
        fs::create_dir_all(&home).unwrap();
        tree(&home);
        fs::create_dir_all(usb.join("Backups")).unwrap();
        fs::write(usb.join("Backups/report-old.pdf"), "x").unwrap();
        let file = d.join("cache/index");
        let stop = AtomicBool::new(false);
        let n = build(&[home.clone(), usb.clone()], &file, &stop).unwrap();
        // Left out on request: Photos.
        let file2 = d.join("cache/index2");
        build_leaving(
            std::slice::from_ref(&home),
            &[home.join("Photos")],
            &file2,
            &stop,
        )
        .unwrap();
        let ix2 = Index::open(&file2).unwrap();
        let none = |_: &Path| true;
        let (hits, _) = ix2.find(&parse("beach"), &any(), &none, &mime, &stop, 10);
        assert!(hits.is_empty(), "a left-out folder isn't searched");
        assert!(n >= 10);
        assert_eq!(
            fs::metadata(&file).unwrap().mode() & 0o777,
            0o600,
            "only yours"
        );
        let ix = Index::open(&file).unwrap();
        assert_eq!(ix.roots(), vec![home.clone(), usb.clone()]);

        let all = |_: &Path| true;
        let (hits, seen) = ix.find(&parse("report"), &any(), &all, &mime, &stop, 100);
        assert!(seen >= 9);
        let mut paths: Vec<String> = hits.iter().map(|h| h.path.clone()).collect();
        paths.sort();
        assert_eq!(
            paths,
            vec![
                home.join("Work/Reports").to_string_lossy().into_owned(),
                home.join("Work/Reports/2024/budget report.pdf")
                    .to_string_lossy()
                    .into_owned(),
                usb.join("Backups/report-old.pdf")
                    .to_string_lossy()
                    .into_owned(),
            ]
        );
        let only_home = |p: &Path| p == home;
        let (hits, _) = ix.find(&parse("*.pdf"), &any(), &only_home, &mime, &stop, 100);
        assert_eq!(hits.len(), 1, "one tree only");
        assert_eq!(hits[0].size, 2_000_000);

        // Gone since: left out.
        fs::remove_file(usb.join("Backups/report-old.pdf")).unwrap();
        let (hits, _) = ix.find(&parse("report-old"), &any(), &all, &mime, &stop, 100);
        assert!(hits.is_empty());
    }

    #[test]
    fn finds_by_name_or_inside() {
        let d = scratch("either");
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("invoice-0412.pdf.txt"), "x").unwrap();
        fs::write(d.join("timesheet.md"), "Sent the invoice to Sam").unwrap();
        fs::write(d.join("other.txt"), "nothing").unwrap();
        let inside = Inside::new("invoice", false, false).unwrap();
        let (q, f) = (parse("invoice"), any());
        let run = |either: bool| {
            let mut got = vec![];
            walk(
                &d,
                &Look {
                    query: &q,
                    filters: &f,
                    hidden: false,
                    mime: &mime,
                    inside: Some(&inside),
                    either,
                },
                &AtomicBool::new(false),
                100,
                &mut |h, _, done| {
                    if done {
                        got = h.iter().map(|x| x.name.clone()).collect();
                    }
                },
            );
            got
        };
        // Either: the name match first, then what has it inside.
        assert_eq!(run(true), ["invoice-0412.pdf.txt", "timesheet.md"]);
        // Both at once: neither has both.
        assert!(run(false).is_empty());
    }

    #[test]
    fn searches_inside_files() {
        let d = scratch("inside");
        fs::create_dir_all(d.join("notes")).unwrap();
        fs::write(
            d.join("notes/meeting.md"),
            "# Monday\n\nAgreed the Invoice total with Sam.\n",
        )
        .unwrap();
        fs::write(d.join("notes/other.txt"), "nothing here").unwrap();
        fs::write(d.join("photo.jpg"), b"\xff\xd8\0invoice").unwrap();
        let mut long = "x ".repeat(200);
        long.push_str("invoice number 42");
        long.push_str(&" y".repeat(200));
        fs::write(d.join("long.log"), &long).unwrap();
        let inside = Inside::new("invoice", false, false).unwrap();
        let (q, f) = (parse(""), any());
        let mut got = vec![];
        walk(
            &d,
            &Look {
                query: &q,
                filters: &f,
                hidden: false,
                mime: &mime,
                inside: Some(&inside),
                either: false,
            },
            &AtomicBool::new(false),
            100,
            &mut |h, _, done| {
                if done {
                    got = h
                        .iter()
                        .map(|x| (x.name.clone(), x.found.clone()))
                        .collect();
                }
            },
        );
        got.sort();
        assert_eq!(
            got.len(),
            2,
            "binary files and non-matches left out: {got:?}"
        );
        assert_eq!(
            got[1],
            (
                "meeting.md".into(),
                "line 3: Agreed the Invoice total with Sam.".into()
            )
        );
        assert!(
            got[0].1.starts_with("line 1: …")
                && got[0].1.contains("invoice number 42")
                && got[0].1.chars().count() <= 112,
            "{:?}",
            got[0]
        );
        // Matching case, and a regular expression.
        assert!(Inside::new("invoice", true, false)
            .unwrap()
            .find(&d.join("notes/meeting.md"), 40, "text/markdown")
            .is_none());
        assert!(Inside::new(r"total\s+with", false, true)
            .unwrap()
            .find(&d.join("notes/meeting.md"), 40, "text/markdown")
            .is_some());
        assert!(Inside::new("(", false, true).is_err());
    }

    #[test]
    fn a_sealed_index_opens_and_searches() {
        let d = scratch("sealed");
        fs::create_dir_all(d.join("Work")).unwrap();
        fs::write(d.join("Work/bank-statement.pdf"), "x").unwrap();
        let f = d.join("index");
        build(&[d.to_path_buf()], &f, &AtomicBool::new(false)).unwrap();
        let plain = fs::read(&f).unwrap();
        crate::secure::set_key_for_tests([3u8; 32]);
        fs::write(&f, crate::secure::seal_with(&[3u8; 32], &plain)).unwrap();
        assert!(
            !fs::read(&f).unwrap().windows(4).any(|w| w == b"bank"),
            "names aren't readable on disk"
        );
        let ix = Index::open(&f).expect("opens sealed");
        assert_eq!(ix.bytes(), &plain[..]);
        // Tampered with: not opened.
        let mut bad = fs::read(&f).unwrap();
        let n = bad.len();
        bad[n - 5] ^= 1;
        fs::write(&f, bad).unwrap();
        assert!(Index::open(&f).is_none());
    }

    #[test]
    fn broken_index_is_none() {
        let d = scratch("broken");
        fs::write(d.join("index"), "not an index at all").unwrap();
        assert!(Index::open(&d.join("index")).is_none());
        assert!(Index::open(&d.join("missing")).is_none());
    }
}
