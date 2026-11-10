//! Tags on files: stored on the file itself as the `user.xdg.tags` extended
//! attribute (comma-separated, the freedesktop.org convention KDE's Dolphin
//! uses too), so they go wherever the file goes, whatever moves it.
//!
//! Files also keeps a list of what's tagged where
//! ($XDG_STATE_HOME/whiterose/files/tags.json) so a tag's files and `tag:`
//! searches are instant. It's kept up as Files tags, renames, moves, trashes
//! and deletes things, filled in from what's found when the search index is
//! made, and checked against the files themselves whenever it's read.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::CString,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

const ATTR: &str = "user.xdg.tags";

fn c_path(p: &Path) -> Option<CString> {
    CString::new(p.as_os_str().as_bytes()).ok()
}

/// "a,b, c" → ["a", "b", "c"] (blanks dropped, each once).
pub fn parse(text: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for t in text.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        if !out.iter().any(|x| x == t) {
            out.push(t.to_string());
        }
    }
    out
}

/// A tag's name, checked: not blank, no commas, not too long.
pub fn check_name(name: &str) -> Result<String, String> {
    let n = name.trim();
    if n.is_empty() {
        return Err("a tag needs a name".into());
    }
    if n.contains(',') || n.chars().any(char::is_control) {
        return Err("tags can't contain commas".into());
    }
    if n.len() > 60 {
        return Err("that name is too long".into());
    }
    Ok(n.to_string())
}

/// The tags on a file (none if it has none, or its drive can't hold them).
pub fn read(p: &Path) -> Vec<String> {
    let Some(c) = c_path(p) else { return vec![] };
    let name = CString::new(ATTR).unwrap();
    let mut buf = vec![0u8; 1024];
    // SAFETY: valid C strings and a buffer of the given length.
    let n = unsafe {
        libc::getxattr(
            c.as_ptr(),
            name.as_ptr(),
            buf.as_mut_ptr().cast(),
            buf.len(),
        )
    };
    if n <= 0 {
        return vec![];
    }
    parse(&String::from_utf8_lossy(&buf[..n as usize]))
}

/// Puts exactly these tags on a file (none: the attribute goes).
pub fn write(p: &Path, tags: &[String]) -> Result<(), String> {
    let c = c_path(p).ok_or("bad path")?;
    let name = CString::new(ATTR).unwrap();
    // SAFETY: valid C strings and a value of the given length.
    let r = unsafe {
        if tags.is_empty() {
            let r = libc::removexattr(c.as_ptr(), name.as_ptr());
            if r != 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ENODATA) {
                0
            } else {
                r
            }
        } else {
            let v = tags.join(",");
            libc::setxattr(c.as_ptr(), name.as_ptr(), v.as_ptr().cast(), v.len(), 0)
        }
    };
    if r == 0 {
        return Ok(());
    }
    let e = std::io::Error::last_os_error();
    Err(match e.raw_os_error() {
        Some(libc::ENOTSUP) => "this drive can't hold tags".into(),
        Some(libc::EACCES) | Some(libc::EPERM) => "permission denied".into(),
        _ => e.to_string(),
    })
}

/// The comment on a file (user.xdg.comment, as other file managers keep it).
pub fn comment(p: &Path) -> String {
    let Some(c) = c_path(p) else {
        return String::new();
    };
    let name = CString::new("user.xdg.comment").unwrap();
    let mut buf = vec![0u8; 4096];
    // SAFETY: valid C strings and a buffer of the given length.
    let n = unsafe {
        libc::getxattr(
            c.as_ptr(),
            name.as_ptr(),
            buf.as_mut_ptr().cast(),
            buf.len(),
        )
    };
    if n <= 0 {
        return String::new();
    }
    String::from_utf8_lossy(&buf[..n as usize]).into_owned()
}

/// Sets (or, empty, removes) a file's comment.
pub fn set_comment(p: &Path, text: &str) -> Result<(), String> {
    let c = c_path(p).ok_or("bad path")?;
    let name = CString::new("user.xdg.comment").unwrap();
    let text = text.trim();
    if text.len() > 4000 {
        return Err("That's too long for a comment".into());
    }
    // SAFETY: valid C strings and a value of the given length.
    let r = unsafe {
        if text.is_empty() {
            let r = libc::removexattr(c.as_ptr(), name.as_ptr());
            if r != 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ENODATA) {
                0
            } else {
                r
            }
        } else {
            libc::setxattr(
                c.as_ptr(),
                name.as_ptr(),
                text.as_ptr().cast(),
                text.len(),
                0,
            )
        }
    };
    if r == 0 {
        return Ok(());
    }
    let e = std::io::Error::last_os_error();
    Err(match e.raw_os_error() {
        Some(libc::ENOTSUP) => "this drive can't hold comments".into(),
        Some(libc::EACCES) | Some(libc::EPERM) => "permission denied".into(),
        _ => e.to_string(),
    })
}

/// Copies the tags of `from` onto `to` (when Files copies a file).
pub fn copy(from: &Path, to: &Path) {
    let t = read(from);
    if !t.is_empty() {
        let _ = write(to, &t);
    }
}

/// What's tagged where (paths → their tags).
#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
#[serde(default)]
pub struct Registry {
    pub files: BTreeMap<String, Vec<String>>,
}

impl Registry {
    pub fn set(&mut self, path: &str, tags: Vec<String>) {
        if tags.is_empty() {
            self.files.remove(path);
        } else {
            self.files.insert(path.to_string(), tags);
        }
    }

    /// `from` (and what's in it) is now at `to`.
    pub fn moved(&mut self, from: &str, to: &str) -> bool {
        let inside = format!("{from}/");
        let keys: Vec<String> = self
            .files
            .keys()
            .filter(|k| *k == from || k.starts_with(&inside))
            .cloned()
            .collect();
        for k in &keys {
            if let Some(t) = self.files.remove(k) {
                self.files.insert(format!("{to}{}", &k[from.len()..]), t);
            }
        }
        !keys.is_empty()
    }

    pub fn forget(&mut self, path: &str) -> bool {
        let inside = format!("{path}/");
        let before = self.files.len();
        self.files
            .retain(|k, _| k != path && !k.starts_with(&inside));
        before != self.files.len()
    }

    /// A tag renamed (`to` None: taken off) on every file listed.
    pub fn rename(&mut self, from: &str, to: Option<&str>) -> Vec<(String, Vec<String>)> {
        let mut changed = vec![];
        for (p, tags) in self.files.iter_mut() {
            if let Some(i) = tags.iter().position(|t| t == from) {
                match to {
                    Some(n) if !tags.iter().any(|t| t == n) => tags[i] = n.to_string(),
                    _ => {
                        tags.remove(i);
                    }
                }
                changed.push((p.clone(), tags.clone()));
            }
        }
        self.files.retain(|_, t| !t.is_empty());
        changed
    }

    /// Files with this tag (as the registry has it).
    pub fn with(&self, tag: &str) -> Vec<String> {
        self.files
            .iter()
            .filter(|(_, t)| t.iter().any(|x| x == tag))
            .map(|(p, _)| p.clone())
            .collect()
    }

    /// How many files have each tag.
    pub fn counts(&self) -> BTreeMap<String, usize> {
        let mut m = BTreeMap::new();
        for t in self.files.values().flatten() {
            *m.entry(t.clone()).or_insert(0) += 1;
        }
        m
    }
}

pub fn file() -> PathBuf {
    crate::session::dir().join("tags.json")
}

pub fn load_from(f: &Path) -> Registry {
    std::fs::read_to_string(f)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save_to(f: &Path, r: &Registry) -> Result<(), String> {
    std::fs::create_dir_all(f.parent().ok_or("no folder")?).map_err(|e| e.to_string())?;
    let tmp = f.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string(r).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, f).map_err(|e| e.to_string())
}

pub fn load() -> Registry {
    load_from(&file())
}

/// Loads, changes and saves if `f` says something changed.
pub fn change(f: impl FnOnce(&mut Registry) -> bool) -> bool {
    let path = file();
    let mut r = load_from(&path);
    if !f(&mut r) {
        return true;
    }
    save_to(&path, &r).is_ok()
}

/// The files with `tag`, checked against the files themselves (gone or
/// untagged since: dropped from the list).
pub fn files_with(tag: &str) -> Vec<PathBuf> {
    let reg = load();
    let mut stale = vec![];
    let mut out = vec![];
    for p in reg.with(tag) {
        let pb = PathBuf::from(&p);
        let now = read(&pb);
        if now.iter().any(|t| t == tag) {
            out.push(pb);
        } else {
            stale.push((p, now));
        }
    }
    if !stale.is_empty() {
        change(|r| {
            for (p, now) in &stale {
                r.set(p, now.clone());
            }
            true
        });
    }
    out
}

/// Tags found on files (by the index) added to the list.
pub fn found(found: Vec<(String, Vec<String>)>) {
    if found.is_empty() {
        return;
    }
    change(|r| {
        let mut any = false;
        for (p, t) in found {
            if r.files.get(&p) != Some(&t) {
                r.set(&p, t);
                any = true;
            }
        }
        any
    });
}

/// Every tag in the list, with how many files have it.
pub fn counts() -> BTreeMap<String, usize> {
    load().counts()
}

/// The set of tags `paths` have between them (for menus).
pub fn on(paths: &[PathBuf]) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for p in paths {
        for t in read(p).into_iter().collect::<BTreeSet<_>>() {
            *m.entry(t).or_insert(0) += 1;
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_on_files() {
        let d = scratch("comment");
        let f = d.join("note.txt");
        std::fs::write(&f, "x").unwrap();
        match set_comment(&f, "  Plans for spring  ") {
            // (A drive without extended attributes: nothing to check.)
            Err(e) if e.contains("can't hold") => return,
            r => r.unwrap(),
        }
        assert_eq!(comment(&f), "Plans for spring");
        set_comment(&f, "").unwrap();
        assert_eq!(comment(&f), "");
        set_comment(&f, "").unwrap(); // removing nothing is fine
        assert!(set_comment(&f, &"x".repeat(5000)).is_err());
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("wr-tags-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn names() {
        assert_eq!(parse(" work, Photos ,,work"), vec!["work", "Photos"]);
        assert!(check_name("a,b").is_err());
        assert!(check_name("  ").is_err());
        assert_eq!(check_name(" Holiday 2026 ").unwrap(), "Holiday 2026");
    }

    #[test]
    fn on_the_file_itself() {
        let d = scratch("xattr");
        let f = d.join("a.txt");
        std::fs::write(&f, "x").unwrap();
        // Some test machines' temp drives can't hold extended attributes.
        if write(&f, &["work".into()]).is_err() {
            eprintln!("(no user xattrs on {}: skipped)", d.display());
            return;
        }
        assert_eq!(read(&f), vec!["work"]);
        write(&f, &["work".into(), "urgent".into()]).unwrap();
        // A move keeps them.
        std::fs::rename(&f, d.join("b.txt")).unwrap();
        assert_eq!(read(&d.join("b.txt")), vec!["work", "urgent"]);
        // A copy made by Files carries them.
        std::fs::write(d.join("c.txt"), "x").unwrap();
        copy(&d.join("b.txt"), &d.join("c.txt"));
        assert_eq!(read(&d.join("c.txt")), vec!["work", "urgent"]);
        write(&d.join("b.txt"), &[]).unwrap();
        assert!(read(&d.join("b.txt")).is_empty());
        write(&d.join("b.txt"), &[]).unwrap();
    }

    #[test]
    fn the_list() {
        let mut r = Registry::default();
        r.set("/h/Work/a.pdf", vec!["work".into(), "urgent".into()]);
        r.set("/h/Work/b.pdf", vec!["work".into()]);
        r.set("/h/Workshop/c.pdf", vec!["work".into()]);
        assert_eq!(r.with("work").len(), 3);
        assert_eq!(r.counts()["urgent"], 1);
        assert!(r.moved("/h/Work", "/h/Jobs"));
        assert_eq!(r.with("urgent"), vec!["/h/Jobs/a.pdf"]);
        assert!(
            r.files.contains_key("/h/Workshop/c.pdf"),
            "a name that only starts the same stays"
        );
        let changed = r.rename("work", Some("job"));
        assert_eq!(changed.len(), 3);
        assert_eq!(r.files["/h/Jobs/a.pdf"], vec!["job", "urgent"]);
        r.rename("urgent", None);
        assert_eq!(r.files["/h/Jobs/a.pdf"], vec!["job"]);
        assert!(r.forget("/h/Jobs"));
        assert_eq!(r.files.len(), 1);
        r.set("/h/Workshop/c.pdf", vec![]);
        assert!(r.files.is_empty(), "no tags: off the list");
    }
}
