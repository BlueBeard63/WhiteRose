//! Clean up ("cleanup:"): finding what takes room under a folder, to trash
//! or delete. Three scans, each run in the background when asked and
//! staying on the folder's own drive:
//!   - the largest files;
//!   - duplicates: same size, then the same first 64 KB, then the same
//!     contents (SHA-256), files of 1 MB and up;
//!   - build caches and downloaded dependencies: folders a build makes again
//!     (node_modules beside a package.json, target beside a Cargo.toml,
//!     virtualenvs, __pycache__...) and the big download caches in home.

use std::{
    collections::{BinaryHeap, HashMap},
    fs,
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Found {
    pub path: String,
    pub size: u64,
    /// unix ms
    pub modified: i64,
    /// What it is (build caches: "Rust build output"; else "").
    pub what: String,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Group {
    pub size: u64,
    pub files: Vec<Found>,
}

/// How far a scan has got (folders and files looked at).
pub type Tell<'a> = &'a mut dyn FnMut(u64);

fn found(p: &Path, m: &fs::Metadata, size: u64, what: &str) -> Found {
    Found {
        path: p.to_string_lossy().into_owned(),
        size,
        modified: m.mtime() * 1000,
        what: what.into(),
    }
}

/// Walks the files under `root` (on its drive; links not followed),
/// calling `each`. False once stopped.
fn walk(
    root: &Path,
    stop: &AtomicBool,
    tell: Tell,
    each: &mut dyn FnMut(&Path, &fs::Metadata) -> bool,
) -> bool {
    let Ok(top) = fs::symlink_metadata(root) else {
        return true;
    };
    let dev = top.dev();
    let mut stack = vec![root.to_path_buf()];
    let mut n = 0u64;
    while let Some(d) = stack.pop() {
        let Ok(rd) = fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            n += 1;
            if n.is_multiple_of(2048) {
                if stop.load(Ordering::Relaxed) {
                    return false;
                }
                tell(n);
            }
            let Ok(m) = e.metadata() else { continue };
            let p = e.path();
            if m.is_dir() {
                // (e.metadata() doesn't follow links: a link is never a dir here.)
                if m.dev() == dev && each(&p, &m) {
                    stack.push(p);
                }
            } else if m.is_file() {
                each(&p, &m);
            }
        }
    }
    tell(n);
    true
}

/// The `keep` largest files under `root`, largest first.
pub fn largest(root: &Path, keep: usize, stop: &AtomicBool, tell: Tell) -> Vec<Found> {
    // A min-heap of the largest so far.
    let mut heap: BinaryHeap<std::cmp::Reverse<(u64, String, i64)>> = BinaryHeap::new();
    walk(root, stop, tell, &mut |p, m| {
        if m.is_file() {
            let item =
                std::cmp::Reverse((m.len(), p.to_string_lossy().into_owned(), m.mtime() * 1000));
            if heap.len() < keep {
                heap.push(item);
            } else if heap.peek().is_some_and(|s| s.0 .0 < m.len()) {
                heap.pop();
                heap.push(item);
            }
        }
        true
    });
    let mut v: Vec<Found> = heap
        .into_iter()
        .map(|std::cmp::Reverse((size, path, modified))| Found {
            path,
            size,
            modified,
            what: String::new(),
        })
        .collect();
    v.sort_by(|a, b| b.size.cmp(&a.size).then(a.path.cmp(&b.path)));
    v
}

fn sum_of(p: &Path, limit: Option<usize>) -> Option<[u8; 32]> {
    let mut f = fs::File::open(p).ok()?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut left = limit.unwrap_or(usize::MAX);
    while left > 0 {
        let want = buf.len().min(left);
        let n = f.read(&mut buf[..want]).ok()?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        left -= n;
    }
    Some(h.finalize().into())
}

/// Files with the same contents under `root` (each `min` bytes or more):
/// the groups wasting most first.
pub fn duplicates(root: &Path, min: u64, stop: &AtomicBool, tell: Tell) -> Vec<Group> {
    let mut by_size: HashMap<u64, Vec<(PathBuf, fs::Metadata)>> = HashMap::new();
    walk(root, stop, tell, &mut |p, m| {
        if m.is_file() && m.len() >= min {
            // (Hard links to one file aren't copies.)
            by_size
                .entry(m.len())
                .or_default()
                .push((p.to_path_buf(), m.clone()));
        }
        true
    });
    let mut groups = vec![];
    for (size, files) in by_size.into_iter().filter(|(_, f)| f.len() > 1) {
        if stop.load(Ordering::Relaxed) {
            return vec![];
        }
        let mut seen_inode = std::collections::HashSet::new();
        let files: Vec<_> = files
            .into_iter()
            .filter(|(_, m)| seen_inode.insert((m.dev(), m.ino())))
            .collect();
        // The start first (cheap), then the whole of those that still match.
        let mut by_head: HashMap<[u8; 32], Vec<(PathBuf, fs::Metadata)>> = HashMap::new();
        for (p, m) in files {
            if let Some(h) = sum_of(&p, Some(1 << 16)) {
                by_head.entry(h).or_default().push((p, m));
            }
        }
        for same_head in by_head.into_values().filter(|v| v.len() > 1) {
            let mut by_all: HashMap<[u8; 32], Vec<Found>> = HashMap::new();
            for (p, m) in same_head {
                let h = if size <= 1 << 16 {
                    sum_of(&p, Some(1 << 16))
                } else {
                    sum_of(&p, None)
                };
                if let Some(h) = h {
                    by_all.entry(h).or_default().push(found(&p, &m, size, ""));
                }
            }
            for mut files in by_all.into_values().filter(|v| v.len() > 1) {
                // Oldest first: likely the original.
                files.sort_by(|a, b| a.modified.cmp(&b.modified).then(a.path.cmp(&b.path)));
                groups.push(Group { size, files });
            }
        }
    }
    groups.sort_by(|a, b| {
        (b.size * (b.files.len() as u64 - 1))
            .cmp(&(a.size * (a.files.len() as u64 - 1)))
            .then(a.files[0].path.cmp(&b.files[0].path))
    });
    groups
}

/// A folder a build makes again, by its name and what's beside or in it:
/// what it is, or None.
pub fn cache_kind(dir: &Path) -> Option<&'static str> {
    let name = dir.file_name()?.to_str()?;
    let parent = dir.parent()?;
    let beside = |f: &str| parent.join(f).exists();
    let inside = |f: &str| dir.join(f).exists();
    Some(match name {
        "node_modules" if beside("package.json") => "JavaScript dependencies",
        "target" if beside("Cargo.toml") => "Rust build output",
        ".venv" | "venv" | "env" if inside("pyvenv.cfg") => "Python virtual environment",
        "__pycache__" => "Python bytecode",
        ".pytest_cache" | ".mypy_cache" | ".ruff_cache" | ".tox" => "Python tool cache",
        ".next" if beside("package.json") => "Next.js build",
        ".nuxt" | ".output" if beside("package.json") => "Nuxt build",
        ".parcel-cache" | ".turbo" | ".svelte-kit" | ".angular" if beside("package.json") => {
            "JavaScript build cache"
        }
        ".gradle"
            if beside("build.gradle")
                || beside("build.gradle.kts")
                || beside("settings.gradle")
                || beside("settings.gradle.kts") =>
        {
            "Gradle cache"
        }
        "build" if beside("build.gradle") || beside("build.gradle.kts") => "Gradle build output",
        ".zig-cache" | "zig-cache" | "zig-out" if beside("build.zig") => "Zig build output",
        ".dart_tool" if beside("pubspec.yaml") => "Dart tool cache",
        "obj" | "bin"
            if std::fs::read_dir(parent)
                .ok()?
                .flatten()
                .any(|e| e.file_name().to_string_lossy().ends_with(".csproj")) =>
        {
            ".NET build output"
        }
        _ => return None,
    })
}

/// The big download caches in home, rebuilt when needed.
pub fn home_caches(home: &Path) -> Vec<(PathBuf, &'static str)> {
    [
        (".cache/pip", "pip downloads"),
        (".npm/_cacache", "npm downloads"),
        (".cache/yarn", "Yarn downloads"),
        (".local/share/pnpm/store", "pnpm store"),
        (".cargo/registry/cache", "Cargo downloads"),
        (".cargo/registry/src", "Cargo sources"),
        ("go/pkg/mod", "Go modules"),
        (".cache/go-build", "Go build cache"),
        (".gradle/caches", "Gradle downloads"),
        (".m2/repository", "Maven downloads"),
        (".nuget/packages", "NuGet packages"),
        (".cache/thumbnails", "Picture thumbnails"),
    ]
    .into_iter()
    .map(|(p, w)| (home.join(p), w))
    .filter(|(p, _)| p.is_dir())
    .collect()
}

/// Bytes under `dir` (on its drive).
pub fn size_of(dir: &Path, stop: &AtomicBool) -> u64 {
    let mut total = 0u64;
    walk(dir, stop, &mut |_| {}, &mut |_, m| {
        if m.is_file() {
            total += m.blocks() * 512;
        }
        true
    });
    total
}

/// Build caches under `root` (and, when `home` is under it, home's download
/// caches), largest first. A cache isn't looked inside.
pub fn caches(root: &Path, home: &Path, stop: &AtomicBool, tell: Tell) -> Vec<Found> {
    let mut hits: Vec<(PathBuf, fs::Metadata, &'static str)> = vec![];
    let extra = if home.starts_with(root) || root.starts_with(home) {
        home_caches(home)
    } else {
        vec![]
    };
    walk(root, stop, tell, &mut |p, m| {
        if !m.is_dir() {
            return true;
        }
        if let Some(what) = cache_kind(p) {
            hits.push((p.to_path_buf(), m.clone(), what));
            return false;
        }
        // Home's download caches are listed whole, not walked into.
        !extra.iter().any(|(x, _)| x == p)
    });
    for (p, what) in extra {
        if p.starts_with(root) {
            if let Ok(m) = fs::metadata(&p) {
                hits.push((p, m, what));
            }
        }
    }
    let mut out: Vec<Found> = hits
        .into_iter()
        .filter_map(|(p, m, what)| {
            if stop.load(Ordering::Relaxed) {
                return None;
            }
            let size = size_of(&p, stop);
            (size > 0).then(|| found(&p, &m, size, what))
        })
        .collect();
    out.sort_by(|a, b| b.size.cmp(&a.size).then(a.path.cmp(&b.path)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    fn scratch(name: &str) -> Scratch {
        let d = Scratch::new(&format!("clean-{name}"));
        d
    }

    #[test]
    fn largest_first() {
        let d = scratch("large");
        fs::create_dir_all(d.join("a/b")).unwrap();
        for (p, n) in [
            ("small", 10),
            ("a/mid", 2000),
            ("a/b/big", 9000),
            ("a/b/.hidden", 5000),
        ] {
            fs::write(d.join(p), vec![1u8; n]).unwrap();
        }
        let stop = AtomicBool::new(false);
        let v = largest(&d, 3, &stop, &mut |_| {});
        let names: Vec<_> = v
            .iter()
            .map(|f| {
                Path::new(&f.path)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(names, vec!["big", ".hidden", "mid"]);
    }

    #[test]
    fn duplicates_by_contents() {
        let d = scratch("dups");
        fs::create_dir_all(d.join("x")).unwrap();
        let a: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        let mut b = a.clone();
        *b.last_mut().unwrap() ^= 1; // same size and start, different end
        fs::write(d.join("one.bin"), &a).unwrap();
        fs::write(d.join("x/copy.bin"), &a).unwrap();
        fs::write(d.join("x/almost.bin"), &b).unwrap();
        fs::hard_link(d.join("one.bin"), d.join("x/link.bin")).unwrap();
        fs::write(d.join("tiny1"), "same").unwrap();
        fs::write(d.join("tiny2"), "same").unwrap();
        let stop = AtomicBool::new(false);
        let g = duplicates(&d, 1000, &stop, &mut |_| {});
        assert_eq!(g.len(), 1, "{g:?}");
        assert_eq!(g[0].files.len(), 2, "the hard link isn't a copy: {g:?}");
        assert!(g[0].files.iter().all(|f| !f.path.ends_with("almost.bin")));
        // Small files counted when asked.
        assert_eq!(duplicates(&d, 1, &stop, &mut |_| {}).len(), 2);
    }

    #[test]
    fn build_caches_by_what_is_beside_them() {
        let d = scratch("caches");
        let home = d.join("home");
        for p in [
            "web/node_modules/left-pad",
            "rust/target/debug",
            "notrust/target",
            "py/.venv/lib",
            "py/pkg/__pycache__",
            ".cache/pip/wheels",
        ] {
            fs::create_dir_all(home.join(p)).unwrap();
        }
        fs::write(home.join("web/package.json"), "{}").unwrap();
        fs::write(
            home.join("web/node_modules/left-pad/index.js"),
            "x".repeat(5000),
        )
        .unwrap();
        fs::write(home.join("rust/Cargo.toml"), "").unwrap();
        fs::write(home.join("rust/target/debug/app"), "x".repeat(9000)).unwrap();
        fs::write(home.join("notrust/target/keep"), "mine").unwrap();
        fs::write(home.join("py/.venv/pyvenv.cfg"), "home = /usr").unwrap();
        fs::write(home.join("py/pkg/__pycache__/m.pyc"), "x").unwrap();
        fs::write(home.join(".cache/pip/wheels/w.whl"), "x".repeat(3000)).unwrap();
        let stop = AtomicBool::new(false);
        let v = caches(&home, &home, &stop, &mut |_| {});
        let got: Vec<(String, &str)> = v
            .iter()
            .map(|f| {
                (
                    f.path
                        .trim_start_matches(&*home.to_string_lossy())
                        .to_string(),
                    f.what.as_str(),
                )
            })
            .collect();
        assert!(
            got.contains(&("/rust/target".into(), "Rust build output")),
            "{got:?}"
        );
        assert!(got.contains(&("/web/node_modules".into(), "JavaScript dependencies")));
        assert!(got.contains(&("/py/.venv".into(), "Python virtual environment")));
        assert!(got.contains(&("/py/pkg/__pycache__".into(), "Python bytecode")));
        assert!(got.contains(&("/.cache/pip".into(), "pip downloads")));
        assert!(
            !got.iter().any(|(p, _)| p.starts_with("/notrust")),
            "a target folder without a Cargo.toml is yours"
        );
        assert_eq!(got[0].0, "/rust/target", "largest first");
    }
}
