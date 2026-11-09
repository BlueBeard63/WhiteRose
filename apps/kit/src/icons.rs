//! Icons the freedesktop way, for any WhiteRose app: a file's type from the
//! shared-mime-info database (globs2), the icon name for that type (or its
//! generic icon), and the image in the icon theme (your theme, then what it
//! inherits, then hicolor) nearest the size asked for. Lookups are cached.

use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

// ---- MIME types ---------------------------------------------------------------

#[derive(Debug, Default)]
pub struct MimeDb {
    /// "*.tar.gz" style patterns, longest first, and exact names.
    globs: Vec<(u32, String, String)>,
    /// mime type -> generic icon name (from generic-icons).
    generic: HashMap<String, String>,
    /// mime type -> icon name (from the icons file).
    icons: HashMap<String, String>,
}

impl MimeDb {
    pub fn load(dirs: &[PathBuf]) -> MimeDb {
        let mut db = MimeDb::default();
        for d in dirs.iter().rev() {
            let mime = d.join("mime");
            if let Ok(t) = fs::read_to_string(mime.join("globs2")) {
                for line in t.lines().filter(|l| !l.starts_with('#')) {
                    let mut f = line.splitn(4, ':');
                    let (Some(w), Some(m), Some(p)) = (f.next(), f.next(), f.next()) else {
                        continue;
                    };
                    db.globs
                        .push((w.parse().unwrap_or(50), m.to_string(), p.to_string()));
                }
            }
            for (file, map) in [("generic-icons", &mut db.generic), ("icons", &mut db.icons)] {
                if let Ok(t) = fs::read_to_string(mime.join(file)) {
                    for (m, i) in t.lines().filter_map(|l| l.split_once(':')) {
                        map.insert(m.to_string(), i.to_string());
                    }
                }
            }
        }
        // Heavier weight first, then longer (more specific) patterns.
        db.globs
            .sort_by(|a, b| b.0.cmp(&a.0).then(b.2.len().cmp(&a.2.len())));
        db
    }

    /// The MIME type for a file name ("application/octet-stream" if unknown).
    pub fn type_of(&self, name: &str) -> &str {
        let lower = name.to_lowercase();
        for (_, mime, pat) in &self.globs {
            if glob_match(pat, name) || glob_match(&pat.to_lowercase(), &lower) {
                return mime;
            }
        }
        "application/octet-stream"
    }

    /// Icon names to try for a MIME type, most specific first.
    pub fn icon_names(&self, mime: &str) -> Vec<String> {
        let mut v = Vec::new();
        if let Some(i) = self.icons.get(mime) {
            v.push(i.clone());
        }
        v.push(mime.replace('/', "-"));
        if let Some(g) = self.generic.get(mime) {
            v.push(g.clone());
        }
        let media = mime.split('/').next().unwrap_or("");
        v.push(format!("{media}-x-generic"));
        v.push("text-x-generic".into());
        v
    }
}

/// `*` and `?` and `[...]` globs, as shared-mime-info uses them.
pub fn glob_match(pat: &str, s: &str) -> bool {
    fn m(p: &[char], s: &[char]) -> bool {
        match (p.first(), s.first()) {
            (None, None) => true,
            (Some('*'), _) => m(&p[1..], s) || (!s.is_empty() && m(p, &s[1..])),
            (Some('?'), Some(_)) => m(&p[1..], &s[1..]),
            (Some('['), Some(c)) => match p.iter().position(|x| *x == ']') {
                Some(end) => p[1..end].contains(c) && m(&p[end + 1..], &s[1..]),
                None => false,
            },
            (Some(a), Some(b)) if a == b => m(&p[1..], &s[1..]),
            _ => false,
        }
    }
    let p: Vec<char> = pat.chars().collect();
    let s: Vec<char> = s.chars().collect();
    m(&p, &s)
}

// ---- icon themes -------------------------------------------------------------------

#[derive(Debug, Clone)]
struct IconDir {
    path: String,
    size: u32,
    scale: u32,
    kind: String, // Fixed | Scalable | Threshold
    min: u32,
    max: u32,
    threshold: u32,
}

#[derive(Debug, Clone)]
struct Theme {
    roots: Vec<PathBuf>,
    dirs: Vec<IconDir>,
    inherits: Vec<String>,
}

fn parse_theme(roots: Vec<PathBuf>, index: &str) -> Theme {
    let mut sections: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut cur = String::new();
    for line in index.lines() {
        let l = line.trim();
        if l.starts_with('[') && l.ends_with(']') {
            cur = l[1..l.len() - 1].to_string();
        } else if let Some((k, v)) = l.split_once('=') {
            sections
                .entry(cur.clone())
                .or_default()
                .insert(k.trim().into(), v.trim().into());
        }
    }
    let head = sections.get("Icon Theme").cloned().unwrap_or_default();
    let list = |k: &str| -> Vec<String> {
        head.get(k)
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut dirs = Vec::new();
    for d in list("Directories")
        .into_iter()
        .chain(list("ScaledDirectories"))
    {
        let Some(s) = sections.get(&d) else { continue };
        let num = |k: &str, def: u32| s.get(k).and_then(|v| v.parse().ok()).unwrap_or(def);
        let size = num("Size", 0);
        if size == 0 {
            continue;
        }
        dirs.push(IconDir {
            path: d.clone(),
            size,
            scale: num("Scale", 1),
            kind: s.get("Type").cloned().unwrap_or_else(|| "Threshold".into()),
            min: num("MinSize", size),
            max: num("MaxSize", size),
            threshold: num("Threshold", 2),
        });
    }
    Theme {
        roots,
        dirs,
        inherits: list("Inherits"),
    }
}

impl IconDir {
    fn matches(&self, size: u32, scale: u32) -> bool {
        if self.scale != scale {
            return false;
        }
        match self.kind.as_str() {
            "Fixed" => self.size == size,
            "Scalable" => self.min <= size && size <= self.max,
            _ => self.size.abs_diff(size) <= self.threshold,
        }
    }
    fn distance(&self, size: u32, scale: u32) -> u32 {
        let (s, want) = (self.size * self.scale, size * scale);
        match self.kind.as_str() {
            "Scalable" if self.min * self.scale <= want && want <= self.max * self.scale => 0,
            "Scalable" => (self.min * self.scale)
                .abs_diff(want)
                .min((self.max * self.scale).abs_diff(want)),
            _ => s.abs_diff(want),
        }
    }
}

pub struct Icons {
    base: Vec<PathBuf>,
    themes: Mutex<HashMap<String, Option<Theme>>>,
    theme: String,
    cache: Mutex<HashMap<(String, u32), Option<PathBuf>>>,
    pub mime: MimeDb,
}

impl Icons {
    pub fn new(theme: &str, base: Vec<PathBuf>, data_dirs: &[PathBuf]) -> Icons {
        Icons {
            base,
            themes: Default::default(),
            theme: theme.into(),
            cache: Default::default(),
            mime: MimeDb::load(data_dirs),
        }
    }

    fn theme(&self, name: &str) -> Option<Theme> {
        let mut themes = self.themes.lock().unwrap();
        themes
            .entry(name.to_string())
            .or_insert_with(|| {
                let roots: Vec<PathBuf> = self
                    .base
                    .iter()
                    .map(|b| b.join(name))
                    .filter(|p| p.is_dir())
                    .collect();
                let index = roots
                    .iter()
                    .find_map(|r| fs::read_to_string(r.join("index.theme")).ok())?;
                Some(parse_theme(roots, &index))
            })
            .clone()
    }

    /// The chain: the theme, what it inherits (depth first), then hicolor.
    fn chain(&self) -> Vec<Theme> {
        let mut out = Vec::new();
        let mut seen = Vec::new();
        let mut stack = vec![self.theme.clone()];
        while let Some(n) = stack.pop() {
            if seen.contains(&n) {
                continue;
            }
            seen.push(n.clone());
            if let Some(t) = self.theme(&n) {
                for i in t.inherits.iter().rev() {
                    stack.push(i.clone());
                }
                out.push(t);
            }
        }
        if !seen.iter().any(|s| s == "hicolor") {
            if let Some(t) = self.theme("hicolor") {
                out.push(t);
            }
        }
        out
    }

    /// The file for an icon name at `size` (logical px, `scale` 1 or 2).
    pub fn lookup(&self, name: &str, size: u32, scale: u32) -> Option<PathBuf> {
        let key = (format!("{name}@{scale}"), size);
        if let Some(hit) = self.cache.lock().unwrap().get(&key) {
            return hit.clone();
        }
        let found = self.find(name, size, scale);
        self.cache.lock().unwrap().insert(key, found.clone());
        found
    }

    fn find(&self, name: &str, size: u32, scale: u32) -> Option<PathBuf> {
        for t in self.chain() {
            // An exact size first, else the nearest one in this theme.
            let mut best: Option<(u32, PathBuf)> = None;
            for d in &t.dirs {
                for root in &t.roots {
                    for ext in ["svg", "png"] {
                        let p = root.join(&d.path).join(format!("{name}.{ext}"));
                        if p.is_file() {
                            if d.matches(size, scale) {
                                return Some(p);
                            }
                            let dist = d.distance(size, scale);
                            if best.as_ref().is_none_or(|(b, _)| dist < *b) {
                                best = Some((dist, p));
                            }
                        }
                    }
                }
            }
            if let Some((_, p)) = best {
                return Some(p);
            }
        }
        ["svg", "png", "xpm"]
            .iter()
            .map(|e| PathBuf::from(format!("/usr/share/pixmaps/{name}.{e}")))
            .find(|p| p.is_file())
    }

    /// The icon for a file or folder (by name; folders get "folder").
    pub fn for_file(&self, name: &str, is_dir: bool, size: u32, scale: u32) -> Option<PathBuf> {
        if is_dir {
            return self.lookup("folder", size, scale);
        }
        let mime = self.mime.type_of(name).to_string();
        self.mime
            .icon_names(&mime)
            .iter()
            .find_map(|n| self.lookup(n, size, scale))
    }
}

/// The base directories icon themes live in.
pub fn icon_base_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(h) = std::env::var_os("HOME") {
        v.push(PathBuf::from(&h).join(".icons"));
    }
    for d in data_dirs() {
        v.push(d.join("icons"));
    }
    v
}

/// $XDG_DATA_HOME then $XDG_DATA_DIRS.
pub fn data_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    match std::env::var_os("XDG_DATA_HOME") {
        Some(d) => v.push(PathBuf::from(d)),
        None => {
            if let Some(h) = std::env::var_os("HOME") {
                v.push(PathBuf::from(h).join(".local/share"));
            }
        }
    }
    let dirs =
        std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    v.extend(dirs.split(':').filter(|s| !s.is_empty()).map(PathBuf::from));
    v
}

/// Your icon theme (GNOME's setting, which Ubuntu and Fedora set), else Adwaita.
pub fn current_theme() -> String {
    std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "icon-theme"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .trim_matches('\'')
                .to_string()
        })
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "Adwaita".into())
}

/// The shared lookup for this process.
pub fn shared() -> &'static Icons {
    static ICONS: OnceLock<Icons> = OnceLock::new();
    ICONS.get_or_init(|| Icons::new(&current_theme(), icon_base_dirs(), &data_dirs()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globs() {
        assert!(glob_match("*.tar.gz", "backup.tar.gz"));
        assert!(glob_match("Makefile", "Makefile"));
        assert!(glob_match("*.[ch]", "main.c"));
        assert!(!glob_match("*.rs", "main.rs.bak"));
        assert!(glob_match("README*", "README.md"));
    }

    /// A tiny theme tree and mime database, built in a temp dir.
    /// A theme tree of its own per test (tests run in parallel).
    fn fixture(test: &str) -> (PathBuf, Icons) {
        let root = std::env::temp_dir().join(format!("wr-icons-{test}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let mk = |p: &str, body: &str| {
            let f = root.join(p);
            fs::create_dir_all(f.parent().unwrap()).unwrap();
            fs::write(f, body).unwrap();
        };
        mk("icons/Mine/index.theme", "[Icon Theme]\nInherits=Base\nDirectories=48x48/places\n[48x48/places]\nSize=48\nType=Fixed\n");
        mk("icons/Mine/48x48/places/folder.png", "png");
        mk("icons/Base/index.theme", "[Icon Theme]\nDirectories=scalable/mimetypes,16x16/mimetypes\n[scalable/mimetypes]\nSize=64\nMinSize=8\nMaxSize=512\nType=Scalable\n[16x16/mimetypes]\nSize=16\nType=Fixed\n");
        mk("icons/Base/scalable/mimetypes/text-x-generic.svg", "<svg/>");
        mk("icons/Base/16x16/mimetypes/application-pdf.png", "png");
        mk(
            "icons/hicolor/index.theme",
            "[Icon Theme]\nDirectories=48x48/apps\n[48x48/apps]\nSize=48\nType=Threshold\n",
        );
        mk("icons/hicolor/48x48/apps/firefox.png", "png");
        mk("mime/globs2", "50:application/pdf:*.pdf\n50:text/x-python:*.py\n10:text/plain:*.txt\n55:application/x-compressed-tar:*.tar.gz\n");
        mk("mime/generic-icons", "text/x-python:text-x-script\n");
        let icons = Icons::new(
            "Mine",
            vec![root.join("icons")],
            std::slice::from_ref(&root),
        );
        (root, icons)
    }

    #[test]
    fn finds_through_the_inheritance_chain_and_nearest_size() {
        let (root, icons) = fixture("finds_through_the_inheritance_chain_and_nearest_size");
        assert_eq!(
            icons.lookup("folder", 48, 1),
            Some(root.join("icons/Mine/48x48/places/folder.png"))
        );
        // Not in Mine: from Base (scalable fits any size).
        assert_eq!(
            icons.lookup("text-x-generic", 32, 1),
            Some(root.join("icons/Base/scalable/mimetypes/text-x-generic.svg"))
        );
        // Only a 16px PDF icon exists: the nearest is used.
        assert_eq!(
            icons.lookup("application-pdf", 48, 1),
            Some(root.join("icons/Base/16x16/mimetypes/application-pdf.png"))
        );
        // hicolor is always last.
        assert_eq!(
            icons.lookup("firefox", 48, 1),
            Some(root.join("icons/hicolor/48x48/apps/firefox.png"))
        );
        assert_eq!(icons.lookup("nope", 48, 1), None);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_icons_by_type_with_fallbacks() {
        let (root, icons) = fixture("file_icons_by_type_with_fallbacks");
        assert_eq!(icons.mime.type_of("a.PDF"), "application/pdf");
        assert_eq!(
            icons.mime.type_of("x.tar.gz"),
            "application/x-compressed-tar"
        );
        assert_eq!(
            icons.mime.type_of("mystery.bin"),
            "application/octet-stream"
        );
        assert_eq!(
            icons.for_file("Report.pdf", false, 48, 1),
            Some(root.join("icons/Base/16x16/mimetypes/application-pdf.png"))
        );
        // text/x-python has no icon here: generic text it is.
        assert_eq!(
            icons.for_file("app.py", false, 48, 1),
            Some(root.join("icons/Base/scalable/mimetypes/text-x-generic.svg"))
        );
        assert_eq!(
            icons.for_file("Music", true, 48, 1),
            Some(root.join("icons/Mine/48x48/places/folder.png"))
        );
        let _ = fs::remove_dir_all(root);
    }
}
