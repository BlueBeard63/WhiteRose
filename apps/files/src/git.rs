//! Git: which repository a folder is in, its branch, and how far that is
//! ahead of or behind its remote, for Files' status bar. That's all Files
//! shows (git itself is for the terminal and editor), so this never looks
//! at the files: the branch is read from .git/HEAD, and ahead / behind is
//! `git rev-list --count` against the upstream, asked only when there is
//! one, without taking git's locks.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde::Serialize;

#[derive(Serialize, Default, Debug, PartialEq)]
pub struct Status {
    /// The repository's top folder ("" when not in one).
    pub root: String,
    pub branch: String,
    pub ahead: u32,
    pub behind: u32,
}

/// The repository `dir` is in (the folder holding `.git`).
pub fn root_of(dir: &Path) -> Option<PathBuf> {
    crate::launch::git_root(dir)
}

/// The git directory for the top folder `root`: `.git` itself, or where a
/// `.git` file points (a worktree or submodule: "gitdir: …").
fn git_dir(root: &Path) -> Option<PathBuf> {
    let dot = root.join(".git");
    if dot.is_dir() {
        return Some(dot);
    }
    let text = fs::read_to_string(&dot).ok()?;
    let to = text.trim().strip_prefix("gitdir:")?.trim();
    let p = Path::new(to);
    Some(if p.is_absolute() {
        p.to_path_buf()
    } else {
        root.join(p)
    })
}

/// The branch HEAD names ("detached" when it names a commit).
pub fn branch_of(head: &str) -> String {
    match head.trim().strip_prefix("ref:") {
        Some(r) => {
            let r = r.trim();
            r.strip_prefix("refs/heads/").unwrap_or(r).to_string()
        }
        None if head.trim().is_empty() => String::new(),
        None => "detached".into(),
    }
}

/// `git rev-list --left-right --count HEAD...@{upstream}`: "3\t1" → (3, 1).
pub fn counts(out: &str) -> (u32, u32) {
    let mut n = out.split_whitespace().map(|x| x.parse().unwrap_or(0));
    (n.next().unwrap_or(0), n.next().unwrap_or(0))
}

/// Does the branch have an upstream? (Its config says so; nothing run.)
fn has_upstream(git_dir: &Path, branch: &str) -> bool {
    // A worktree's config is its main repository's.
    let common = fs::read_to_string(git_dir.join("commondir"))
        .ok()
        .map(|c| git_dir.join(c.trim()))
        .unwrap_or_else(|| git_dir.to_path_buf());
    let Ok(cfg) = fs::read_to_string(common.join("config")) else {
        return false;
    };
    let section = format!("[branch \"{branch}\"]");
    let mut inside = false;
    for line in cfg.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            inside = l == section;
        } else if inside && l.replace(' ', "").starts_with("merge=") {
            return true;
        }
    }
    false
}

/// The status of `dir` (empty when it isn't in a repository).
pub fn status(dir: &Path) -> Status {
    let Some(root) = root_of(dir) else {
        return Status::default();
    };
    let Some(gd) = git_dir(&root) else {
        return Status::default();
    };
    let branch = branch_of(&fs::read_to_string(gd.join("HEAD")).unwrap_or_default());
    let (mut ahead, mut behind) = (0, 0);
    if !branch.is_empty()
        && branch != "detached"
        && has_upstream(&gd, &branch)
        && crate::launch::installed("git")
    {
        let out = Command::new("git")
            .args([
                "--no-optional-locks",
                "rev-list",
                "--left-right",
                "--count",
                "HEAD...@{upstream}",
            ])
            .current_dir(&root)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output();
        if let Ok(o) = out {
            if o.status.success() {
                (ahead, behind) = counts(&String::from_utf8_lossy(&o.stdout));
            }
        }
    }
    Status {
        root: root.to_string_lossy().into_owned(),
        branch,
        ahead,
        behind,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_branch() {
        assert_eq!(branch_of("ref: refs/heads/main\n"), "main");
        assert_eq!(branch_of("ref: refs/heads/feature/x"), "feature/x");
        assert_eq!(branch_of("3f9a1c2b7e44d0a1\n"), "detached");
        assert_eq!(branch_of(""), "");
        assert_eq!(counts("3\t1\n"), (3, 1));
        assert_eq!(counts(""), (0, 0));
    }

    #[test]
    fn a_repository_without_running_git() {
        let d = std::env::temp_dir().join(format!("wr-git-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join(".git")).unwrap();
        fs::create_dir_all(d.join("src/deep")).unwrap();
        fs::write(d.join(".git/HEAD"), "ref: refs/heads/work\n").unwrap();
        // No upstream in its config: nothing is asked of git.
        fs::write(d.join(".git/config"), "[core]\n\tbare = false\n").unwrap();
        let s = status(&d.join("src/deep"));
        assert_eq!((s.branch.as_str(), s.ahead, s.behind), ("work", 0, 0));
        assert_eq!(s.root, d.to_string_lossy());
        assert!(has_upstream(&d.join(".git"), "work") == false);
        fs::write(
            d.join(".git/config"),
            "[branch \"work\"]\n\tremote = origin\n\tmerge = refs/heads/work\n",
        )
        .unwrap();
        assert!(has_upstream(&d.join(".git"), "work"));
        // A worktree: .git is a file pointing at its git directory.
        let w = d.join("wt");
        fs::create_dir_all(d.join(".git/worktrees/wt")).unwrap();
        fs::create_dir_all(&w).unwrap();
        fs::write(
            w.join(".git"),
            format!("gitdir: {}\n", d.join(".git/worktrees/wt").display()),
        )
        .unwrap();
        fs::write(d.join(".git/worktrees/wt/HEAD"), "ref: refs/heads/side\n").unwrap();
        assert_eq!(
            branch_of(&fs::read_to_string(git_dir(&w).unwrap().join("HEAD")).unwrap()),
            "side"
        );
        // Not a repository.
        assert_eq!(
            status(&std::env::temp_dir().join("wr-git-none-here")),
            Status::default()
        );
        fs::remove_dir_all(&d).unwrap();
    }
}
