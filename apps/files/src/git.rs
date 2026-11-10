//! Git: the state of what's in a folder inside a repository (modified, new,
//! untracked, conflicted, ignored), and the branch with how far it is ahead
//! of or behind its remote. From `git status --porcelain=v2`, run without
//! taking git's locks, so it never gets in the way of git itself.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde::Serialize;

/// How a file stands. Worst first: a folder shows the worst inside it.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Conflict,
    Modified,
    Added,
    Untracked,
    Ignored,
}

#[derive(Serialize, Default, Debug, PartialEq)]
pub struct Status {
    /// The repository's top folder ("" when not in one).
    pub root: String,
    pub branch: String,
    pub ahead: u32,
    pub behind: u32,
    /// The whole folder is ignored (inside an ignored folder).
    pub ignored: bool,
    /// Each entry of the folder with a state, by name.
    pub entries: BTreeMap<String, State>,
}

/// Parsed porcelain v2 (`-z`): the branch, ahead / behind, and each path
/// (relative to the root) with its state; ignored folders end in "/".
#[derive(Default, Debug, PartialEq)]
pub struct Raw {
    pub branch: String,
    pub ahead: u32,
    pub behind: u32,
    pub paths: Vec<(String, State)>,
}

fn xy_state(xy: &str) -> State {
    let mut c = xy.chars();
    let (x, y) = (c.next().unwrap_or('.'), c.next().unwrap_or('.'));
    if x == 'A' && (y == '.' || y == 'M') {
        State::Added
    } else {
        State::Modified
    }
}

pub fn parse(out: &[u8]) -> Raw {
    let mut r = Raw::default();
    let fields: Vec<&[u8]> = out.split(|b| *b == 0).collect();
    let mut i = 0;
    while i < fields.len() {
        let line = String::from_utf8_lossy(fields[i]).into_owned();
        i += 1;
        if let Some(h) = line.strip_prefix("# branch.head ") {
            r.branch = if h == "(detached)" {
                "detached".into()
            } else {
                h.into()
            };
        } else if let Some(ab) = line.strip_prefix("# branch.ab ") {
            for p in ab.split(' ') {
                if let Some(n) = p.strip_prefix('+') {
                    r.ahead = n.parse().unwrap_or(0);
                } else if let Some(n) = p.strip_prefix('-') {
                    r.behind = n.parse().unwrap_or(0);
                }
            }
        } else if let Some(p) = line.strip_prefix("? ") {
            r.paths.push((p.into(), State::Untracked));
        } else if let Some(p) = line.strip_prefix("! ") {
            r.paths.push((p.into(), State::Ignored));
        } else if line.starts_with("1 ") {
            // 1 XY sub mH mI mW hH hI path
            let parts: Vec<&str> = line.splitn(9, ' ').collect();
            if parts.len() == 9 {
                r.paths.push((parts[8].into(), xy_state(parts[1])));
            }
        } else if line.starts_with("2 ") {
            // 2 XY sub mH mI mW hH hI score path, then the old path
            let parts: Vec<&str> = line.splitn(10, ' ').collect();
            if parts.len() == 10 {
                r.paths.push((parts[9].into(), State::Modified));
            }
            i += 1;
        } else if line.starts_with("u ") {
            // u XY sub m1 m2 m3 mW h1 h2 h3 path
            let parts: Vec<&str> = line.splitn(11, ' ').collect();
            if parts.len() == 11 {
                r.paths.push((parts[10].into(), State::Conflict));
            }
        }
    }
    r
}

/// The states of the entries of `rel_dir` (relative to the root; "" for
/// the root itself): a file's own, a folder's the worst inside it.
pub fn in_folder(raw: &Raw, rel_dir: &str) -> (bool, BTreeMap<String, State>) {
    let prefix = if rel_dir.is_empty() {
        String::new()
    } else {
        format!("{rel_dir}/")
    };
    let mut ignored = false;
    let mut out: BTreeMap<String, State> = BTreeMap::new();
    for (p, st) in &raw.paths {
        // An ignored folder this one is in (or is).
        if *st == State::Ignored
            && p.ends_with('/')
            && !prefix.is_empty()
            && prefix.starts_with(p.as_str())
        {
            ignored = true;
            continue;
        }
        let Some(rest) = p.strip_prefix(&prefix) else {
            continue;
        };
        let rest = rest.trim_end_matches('/');
        if rest.is_empty() {
            continue;
        }
        let (child, deeper) = match rest.split_once('/') {
            Some((c, _)) => (c, true),
            None => (rest, false),
        };
        // Ignored things deeper down don't make the folder ignored.
        if deeper && *st == State::Ignored {
            continue;
        }
        let e = out.entry(child.to_string()).or_insert(*st);
        if *st < *e {
            *e = *st;
        }
    }
    (ignored, out)
}

/// The repository `dir` is in (the folder holding `.git`).
pub fn root_of(dir: &Path) -> Option<PathBuf> {
    crate::launch::git_root(dir)
}

/// The status of `dir` (empty when it isn't in a repository or git isn't there).
pub fn status(dir: &Path) -> Status {
    let Some(root) = root_of(dir) else {
        return Status::default();
    };
    if !crate::launch::installed("git") {
        return Status::default();
    }
    let out = Command::new("git")
        .args([
            "--no-optional-locks",
            "status",
            "--porcelain=v2",
            "--branch",
            "-z",
            "--ignored=matching",
            "--untracked-files=normal",
        ])
        .current_dir(&root)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    let Ok(out) = out else {
        return Status::default();
    };
    if !out.status.success() {
        return Status::default();
    }
    let raw = parse(&out.stdout);
    let rel = dir
        .strip_prefix(&root)
        .map(|r| r.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (ignored, entries) = in_folder(&raw, &rel);
    Status {
        root: root.to_string_lossy().into_owned(),
        branch: raw.branch,
        ahead: raw.ahead,
        behind: raw.behind,
        ignored,
        entries,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z(lines: &[&str]) -> Vec<u8> {
        let mut v = vec![];
        for l in lines {
            v.extend_from_slice(l.as_bytes());
            v.push(0);
        }
        v
    }

    #[test]
    fn porcelain() {
        let r = parse(&z(&[
            "# branch.oid abc",
            "# branch.head main",
            "# branch.upstream origin/main",
            "# branch.ab +2 -1",
            "1 .M N... 100644 100644 100644 aa bb src/main.rs",
            "1 A. N... 000000 100644 100644 00 cc src/new file.rs",
            "2 R. N... 100644 100644 100644 aa aa R100 docs/b.md",
            "docs/a.md",
            "u UU N... 100644 100644 100644 100644 aa bb cc conflict.txt",
            "? notes.txt",
            "! target/",
            "! src/gen.o",
        ]));
        assert_eq!((r.branch.as_str(), r.ahead, r.behind), ("main", 2, 1));
        assert_eq!(r.paths.len(), 7, "the rename's old path isn't an entry");
        assert!(
            r.paths.contains(&("src/new file.rs".into(), State::Added)),
            "spaces in names"
        );
        assert!(r.paths.contains(&("docs/b.md".into(), State::Modified)));
        assert_eq!(parse(&z(&["# branch.head (detached)"])).branch, "detached");
    }

    #[test]
    fn a_folder_shows_the_worst_inside() {
        let r = parse(&z(&[
            "1 .M N... 1 1 1 a b src/main.rs",
            "1 A. N... 1 1 1 a b src/lib/x.rs",
            "? src/scratch.rs",
            "u UU N... 1 1 1 1 a b c src/lib/y.rs",
            "? README.md",
            "! target/",
            "! src/gen.o",
            "! src/deep/cache/",
        ]));
        let (ign, top) = in_folder(&r, "");
        assert!(!ign);
        assert_eq!(top["src"], State::Conflict, "conflict beats the rest");
        assert_eq!(top["README.md"], State::Untracked);
        assert_eq!(top["target"], State::Ignored);
        let (_, src) = in_folder(&r, "src");
        assert_eq!(src["main.rs"], State::Modified);
        assert_eq!(src["lib"], State::Conflict);
        assert_eq!(src["gen.o"], State::Ignored);
        assert!(
            !src.contains_key("deep"),
            "something ignored deep inside doesn't make the folder ignored"
        );
        let (ign, inside) = in_folder(&r, "target/debug");
        assert!(ign && inside.is_empty(), "inside an ignored folder");
        assert!(
            !in_folder(&r, "targets").0,
            "a name that only starts the same isn't"
        );
    }

    #[test]
    fn a_real_repository() {
        if !crate::launch::installed("git") {
            return;
        }
        let d = std::env::temp_dir().join(format!("wr-git-status-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("src")).unwrap();
        let git = |args: &[&str]| {
            Command::new("git")
                .args(args)
                .current_dir(&d)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .output()
                .unwrap()
        };
        git(&["init", "-q", "-b", "main"]);
        std::fs::write(d.join("src/a.rs"), "a").unwrap();
        std::fs::write(d.join(".gitignore"), "build/\n").unwrap();
        git(&["add", "."]);
        git(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-qm",
            "x",
        ]);
        std::fs::write(d.join("src/a.rs"), "b").unwrap();
        std::fs::write(d.join("new.txt"), "n").unwrap();
        std::fs::create_dir_all(d.join("build")).unwrap();
        std::fs::write(d.join("build/out"), "o").unwrap();
        let s = status(&d);
        assert_eq!(s.branch, "main");
        assert_eq!(s.entries["src"], State::Modified);
        assert_eq!(s.entries["new.txt"], State::Untracked);
        assert_eq!(s.entries["build"], State::Ignored);
        let s = status(&d.join("src"));
        assert_eq!(s.entries["a.rs"], State::Modified);
        assert!(status(&d.join("build")).ignored);
        assert_eq!(status(&std::env::temp_dir()).root, "", "not a repository");
    }
}
