//! Reading a folder: its entries, folders first, in natural order ("file2"
//! before "file10", ignoring case), hidden ones only when asked.

use std::{
    cmp::Ordering,
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Path,
    time::UNIX_EPOCH,
};

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub dir: bool,
    pub link: bool,
    /// Bytes (files); 0 for folders (sized separately, later).
    pub size: u64,
    /// Modified, unix ms.
    pub modified: i64,
    /// Created, unix ms (0: the drive doesn't record it).
    pub created: i64,
    pub hidden: bool,
    pub executable: bool,
    pub mime: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    Name,
    Size,
    Modified,
    Created,
    Kind,
}

/// Where folders go among files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Folders {
    First,
    Mixed,
    Last,
}

impl Folders {
    pub fn parse(s: &str) -> Folders {
        match s {
            "mixed" => Folders::Mixed,
            "last" => Folders::Last,
            _ => Folders::First,
        }
    }
}

/// "file2" < "file10", "Apple" ~ "apple".
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(c), Some(d)) if c.is_ascii_digit() && d.is_ascii_digit() => {
                let mut n1 = String::new();
                while let Some(c) = x.peek().filter(|c| c.is_ascii_digit()) {
                    n1.push(*c);
                    x.next();
                }
                let mut n2 = String::new();
                while let Some(d) = y.peek().filter(|c| c.is_ascii_digit()) {
                    n2.push(*d);
                    y.next();
                }
                let (t1, t2) = (n1.trim_start_matches('0'), n2.trim_start_matches('0'));
                let o = t1.len().cmp(&t2.len()).then_with(|| t1.cmp(t2));
                if o != Ordering::Equal {
                    return o;
                }
            }
            (Some(c), Some(d)) => {
                let o = c.to_lowercase().cmp(d.to_lowercase());
                if o != Ordering::Equal {
                    return o;
                }
                x.next();
                y.next();
            }
        }
    }
}

/// Folders first (what most callers want).
#[cfg(test)]
pub fn sort(entries: &mut [Entry], by: Sort, descending: bool) {
    sort_with(entries, by, descending, Folders::First)
}

pub fn sort_with(entries: &mut [Entry], by: Sort, descending: bool, folders: Folders) {
    entries.sort_by(|a, b| {
        let f = match folders {
            Folders::First => b.dir.cmp(&a.dir),
            Folders::Last => a.dir.cmp(&b.dir),
            Folders::Mixed => Ordering::Equal,
        };
        if f != Ordering::Equal {
            return f;
        }
        let o = match by {
            Sort::Name => natural_cmp(&a.name, &b.name),
            Sort::Size => a
                .size
                .cmp(&b.size)
                .then_with(|| natural_cmp(&a.name, &b.name)),
            Sort::Modified => a
                .modified
                .cmp(&b.modified)
                .then_with(|| natural_cmp(&a.name, &b.name)),
            Sort::Created => a
                .created
                .cmp(&b.created)
                .then_with(|| natural_cmp(&a.name, &b.name)),
            Sort::Kind => a
                .mime
                .cmp(&b.mime)
                .then_with(|| natural_cmp(&a.name, &b.name)),
        };
        if descending {
            o.reverse()
        } else {
            o
        }
    });
}

/// The folder's entries (unsorted). `mime` names a file's type.
pub fn read(dir: &Path, hidden: bool, mime: impl Fn(&str) -> String) -> Result<Vec<Entry>, String> {
    let rd = fs::read_dir(dir).map_err(|e| match e.kind() {
        std::io::ErrorKind::PermissionDenied => {
            "You don't have permission to open this folder".to_string()
        }
        std::io::ErrorKind::NotFound => "This folder doesn't exist".to_string(),
        _ => e.to_string(),
    })?;
    let mut out = Vec::new();
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let is_hidden = name.starts_with('.');
        if is_hidden && !hidden {
            continue;
        }
        let path = e.path();
        let link = e.file_type().map(|t| t.is_symlink()).unwrap_or(false);
        // Follow links for what they point at (a link to a folder opens as one).
        let meta = fs::metadata(&path).or_else(|_| fs::symlink_metadata(&path));
        let Ok(meta) = meta else { continue };
        let dir = meta.is_dir();
        out.push(Entry {
            mime: if dir {
                "inode/directory".into()
            } else {
                mime(&name)
            },
            name,
            path: path.to_string_lossy().into_owned(),
            dir,
            link,
            size: if dir { 0 } else { meta.size() },
            modified: meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0),
            created: meta
                .created()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0),
            hidden: is_hidden,
            executable: !dir && meta.permissions().mode() & 0o111 != 0,
        });
    }
    Ok(out)
}

/// The first `limit` bytes of a file as text (cut at a character boundary),
/// and whether there's more; None if it isn't text (a NUL byte, or not UTF-8).
pub fn text_start(path: &Path, limit: usize) -> Option<(String, bool)> {
    use std::io::Read;
    let f = fs::File::open(path).ok()?;
    let mut buf = Vec::with_capacity(limit.min(1 << 16));
    f.take(limit as u64 + 1).read_to_end(&mut buf).ok()?;
    let more = buf.len() > limit;
    buf.truncate(limit);
    if buf.contains(&0) {
        return None;
    }
    let text = match std::str::from_utf8(&buf) {
        Ok(t) => t.to_string(),
        // Cut in the middle of a character at the end: fine; elsewhere: not text.
        Err(e) if e.error_len().is_none() => {
            String::from_utf8_lossy(&buf[..e.valid_up_to()]).into_owned()
        }
        Err(_) => return None,
    };
    Some((text, more))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(name: &str, dir: bool, size: u64) -> Entry {
        Entry {
            name: name.into(),
            path: format!("/x/{name}"),
            dir,
            link: false,
            size,
            modified: 0,
            created: 0,
            hidden: name.starts_with('.'),
            executable: false,
            mime: String::new(),
        }
    }

    #[test]
    fn natural_order() {
        let mut v = ["file10", "File2", "file1", "apple", "Banana", "file02b"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            v,
            ["apple", "Banana", "file1", "File2", "file02b", "file10"]
        );
    }

    #[test]
    fn folders_first_whatever_the_order() {
        let mut v = vec![
            e("b.txt", false, 5),
            e("Zoo", true, 0),
            e("a.txt", false, 9),
            e("Alpha", true, 0),
        ];
        sort(&mut v, Sort::Size, true);
        let names: Vec<_> = v.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names,
            ["Zoo", "Alpha", "a.txt", "b.txt"],
            "folders first; then largest first"
        );
        sort(&mut v, Sort::Name, false);
        let names: Vec<_> = v.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["Alpha", "Zoo", "a.txt", "b.txt"]);
    }

    #[test]
    fn reads_a_folder() {
        let d = std::env::temp_dir().join(format!("wr-files-list-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("sub")).unwrap();
        fs::write(d.join("a.txt"), "hello").unwrap();
        fs::write(d.join(".secret"), "x").unwrap();
        std::os::unix::fs::symlink(d.join("sub"), d.join("link-to-sub")).unwrap();
        let mut v = read(&d, false, |_| "text/plain".into()).unwrap();
        sort(&mut v, Sort::Name, false);
        let got: Vec<_> = v
            .iter()
            .map(|e| (e.name.as_str(), e.dir, e.link, e.size))
            .collect();
        assert_eq!(
            got,
            [
                ("link-to-sub", true, true, 0),
                ("sub", true, false, 0),
                ("a.txt", false, false, 5)
            ]
        );
        assert_eq!(
            read(&d, true, |_| String::new()).unwrap().len(),
            4,
            "hidden files when asked"
        );
        assert!(read(&d.join("missing"), false, |_| String::new())
            .unwrap_err()
            .contains("doesn't exist"));
        fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn text_starts() {
        let d = std::env::temp_dir().join(format!("wr-text-{}", std::process::id()));
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("a.txt"), "héllo world").unwrap();
        assert_eq!(
            text_start(&d.join("a.txt"), 100),
            Some(("héllo world".into(), false))
        );
        // Cut inside "é" (2 bytes at 1..3): the half character is left off.
        assert_eq!(text_start(&d.join("a.txt"), 2), Some(("h".into(), true)));
        fs::write(d.join("b.bin"), [0x89u8, b'P', b'N', b'G', 0, 1]).unwrap();
        assert_eq!(text_start(&d.join("b.bin"), 100), None);
        fs::write(d.join("c.txt"), [b'a', 0xff, b'b']).unwrap();
        assert_eq!(text_start(&d.join("c.txt"), 100), None, "not UTF-8");
        assert_eq!(text_start(&d.join("missing"), 100), None);
    }

    #[test]
    fn folders_first_mixed_or_last() {
        let e = |n: &str, dir: bool| Entry {
            name: n.into(), path: n.into(), dir, link: false, size: 0, modified: 0, created: 0,
            hidden: false, executable: false, mime: String::new(),
        };
        let names = |v: &[Entry]| v.iter().map(|e| e.name.clone()).collect::<Vec<_>>();
        let mut v = vec![e("b.txt", false), e("Zed", true), e("a.txt", false), e("Apps", true)];
        sort_with(&mut v, Sort::Name, false, Folders::First);
        assert_eq!(names(&v), ["Apps", "Zed", "a.txt", "b.txt"]);
        sort_with(&mut v, Sort::Name, false, Folders::Mixed);
        assert_eq!(names(&v), ["a.txt", "Apps", "b.txt", "Zed"]);
        sort_with(&mut v, Sort::Name, false, Folders::Last);
        assert_eq!(names(&v), ["a.txt", "b.txt", "Apps", "Zed"]);
    }
}
