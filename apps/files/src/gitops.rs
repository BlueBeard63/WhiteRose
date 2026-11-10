//! Git operations from Files: clone, start a repository, fetch, pull,
//! push, sync; switch to, make and delete branches; stage, unstage,
//! commit, and throw away changes (into the Trash, never gone for good).
//!
//! Everything runs the `git` you have, with your config, credential
//! helper and SSH agent. Git never stops to ask on a terminal (there's
//! none): when a remote wants a password it isn't given, that comes back
//! as an error saying to set up a credential helper or an SSH key.

use std::{
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use serde::Serialize;

fn git(dir: &Path) -> Command {
    let mut c = Command::new("git");
    c.current_dir(dir)
        // No terminal to ask on: fail instead of waiting forever.
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes")
        .env("LC_ALL", "C")
        .stdin(Stdio::null());
    c
}

/// Runs git to the end (or `stop`), telling `progress` the percentages it
/// prints on the way (clone, fetch). Its output, or its error said plainly.
pub fn run(
    dir: &Path,
    args: &[&str],
    wait: Duration,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(u32, String),
) -> Result<String, String> {
    let mut child = git(dir)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Can't run git: {e}"))?;
    let mut stdout = child.stdout.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stdout.read_to_string(&mut s);
        s
    });
    let stderr = child.stderr.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        // Progress lines end in \r, not \n.
        let mut r = BufReader::new(stderr);
        let mut buf = vec![];
        loop {
            buf.clear();
            match r.read_until(b'\r', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    for part in String::from_utf8_lossy(&buf).split(['\r', '\n']) {
                        if !part.trim().is_empty() && tx.send(part.to_string()).is_err() {
                            return;
                        }
                    }
                }
            }
        }
    });
    let until = Instant::now() + wait;
    let mut errors: Vec<String> = vec![];
    loop {
        while let Ok(l) = rx.try_recv() {
            if let Some((what, pct)) = progress_of(&l) {
                progress(pct, what);
            } else {
                errors.push(l);
            }
        }
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            while let Ok(l) = rx.try_recv() {
                if progress_of(&l).is_none() {
                    errors.push(l);
                }
            }
            let o = out.join().unwrap_or_default();
            return if status.success() {
                Ok(o)
            } else {
                Err(plain(&errors.join("\n")))
            };
        }
        if stop.load(Ordering::Relaxed) || Instant::now() > until {
            let _ = child.kill();
            let _ = child.wait();
            return Err(if stop.load(Ordering::Relaxed) {
                "Cancelled".into()
            } else {
                "It took too long".into()
            });
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// "Receiving objects:  42% (420/1000)" -> ("Receiving objects", 42).
fn progress_of(line: &str) -> Option<(String, u32)> {
    let l = line.trim().trim_start_matches("remote: ");
    let (what, rest) = l.split_once(": ")?;
    let pct = rest
        .trim_start()
        .split('%')
        .next()?
        .trim()
        .parse::<u32>()
        .ok()?;
    rest.contains('%').then(|| (what.to_string(), pct.min(100)))
}

/// Git's error, said plainly: the "fatal:"/"error:" lines, and what the
/// common ones mean.
pub fn plain(stderr: &str) -> String {
    let lines: Vec<&str> = stderr
        .lines()
        .map(str::trim)
        .filter(|l| {
            l.starts_with("fatal:") || l.starts_with("error:") || l.starts_with("! [rejected]")
        })
        .collect();
    let msg = if lines.is_empty() {
        stderr
            .trim()
            .lines()
            .last()
            .unwrap_or("git failed")
            .to_string()
    } else {
        lines.join("\n")
    };
    let msg = msg.replace("fatal: ", "").replace("error: ", "");
    let hint = [
        ("could not read Username", "it needs signing in: set up a credential helper (e.g. git-credential-libsecret) or use an SSH address with a key"),
        ("terminal prompts disabled", "it needs signing in: set up a credential helper or use an SSH address with a key"),
        ("Permission denied (publickey)", "the server didn't accept your SSH key (is it added to your account and to ssh-agent?)"),
        ("Host key verification failed", "this computer hasn't seen the server before: connect once with ssh in a terminal to check and accept its key"),
        ("[rejected]", "the remote has changes you don't have yet: pull (or sync) first"),
        ("non-fast-forward", "the remote has changes you don't have yet: pull (or sync) first"),
        ("would be overwritten", "you have changes that would be lost: commit them, or put them aside, first"),
        ("has no upstream branch", "this branch isn't on the remote yet: push it to publish it"),
        ("Not possible to fast-forward", "both sides have new commits: use sync, which puts yours after theirs"),
        ("CONFLICT", "some files changed on both sides: they're marked ! to sort out, then commit"),
    ]
    .iter()
    .find(|(k, _)| stderr.contains(k))
    .map(|(_, h)| *h);
    match hint {
        Some(h) => format!("{msg}\n({h})"),
        None => msg,
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Branch {
    pub name: String,
    /// "origin/main", "" if none.
    pub upstream: String,
    pub current: bool,
    /// Only on the remote (switching makes a local branch tracking it).
    pub remote: bool,
}

/// Local branches (current first), then remote ones with no local branch.
pub fn branches(dir: &Path) -> Result<Vec<Branch>, String> {
    let out = git(dir)
        .args([
            "for-each-ref",
            "--format=%(HEAD)%00%(refname)%00%(upstream:short)",
            "refs/heads",
            "refs/remotes",
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(plain(&String::from_utf8_lossy(&out.stderr)));
    }
    Ok(parse_branches(&String::from_utf8_lossy(&out.stdout)))
}

fn parse_branches(text: &str) -> Vec<Branch> {
    let mut local = vec![];
    let mut remote = vec![];
    for l in text.lines() {
        let f: Vec<&str> = l.split('\0').collect();
        let (head, refname, up) = (
            f.first().copied().unwrap_or(""),
            f.get(1).copied().unwrap_or(""),
            f.get(2).copied().unwrap_or(""),
        );
        if let Some(n) = refname.strip_prefix("refs/heads/") {
            local.push(Branch {
                name: n.into(),
                upstream: up.into(),
                current: head == "*",
                remote: false,
            });
        } else if let Some(n) = refname.strip_prefix("refs/remotes/") {
            if !n.ends_with("/HEAD") {
                remote.push(n.to_string());
            }
        }
    }
    local.sort_by(|a, b| b.current.cmp(&a.current).then(a.name.cmp(&b.name)));
    // A remote branch someone already has locally isn't listed twice.
    let tracked: Vec<String> = local.iter().map(|b| b.upstream.clone()).collect();
    let mut out = local;
    for r in remote {
        let short = r.split_once('/').map(|(_, b)| b).unwrap_or(&r);
        if !tracked.contains(&r) && !out.iter().any(|b| !b.remote && b.name == short) {
            out.push(Branch {
                name: r.clone(),
                upstream: r,
                current: false,
                remote: true,
            });
        }
    }
    out
}

/// A name git takes for a branch.
pub fn check_branch(name: &str) -> Result<&str, String> {
    let n = name.trim();
    let ok = Command::new("git")
        .args(["check-ref-format", "--branch", n])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if n.is_empty() || !ok {
        return Err(format!(
            "“{n}” can't be a branch name (no spaces, ~ ^ : ? * [ \\ or ..)"
        ));
    }
    Ok(n)
}

/// The arguments for each operation (`what`), given its inputs.
pub fn args_for(what: &str, arg: &str, paths: &[String]) -> Result<Vec<String>, String> {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    let with_paths = |v: &[&str]| {
        let mut a = s(v);
        a.push("--".into());
        a.extend(paths.iter().cloned());
        a
    };
    Ok(match what {
        "init" => s(&["init"]),
        "fetch" => s(&["fetch", "--all", "--prune", "--progress"]),
        "pull" => s(&["pull", "--ff-only", "--progress"]),
        // Sync: theirs first, yours on top, then yours up.
        "sync" => s(&["pull", "--rebase", "--autostash", "--progress"]),
        "push" => s(&["push", "--progress"]),
        // A branch's first push: published and tracked.
        "publish" => s(&["push", "--set-upstream", "origin", "HEAD", "--progress"]),
        "switch" => s(&["switch", arg]),
        // A branch only on the remote: a local one that tracks it.
        "switch-remote" => s(&["switch", "--track", arg]),
        "branch" => vec![
            "switch".into(),
            "--create".into(),
            check_branch(arg)?.into(),
        ],
        "delete-branch" => vec!["branch".into(), "--delete".into(), arg.into()],
        "stage" => with_paths(&["add"]),
        "unstage" => with_paths(&["restore", "--staged"]),
        "commit" => {
            if arg.trim().is_empty() {
                return Err("Write what the commit does".into());
            }
            vec!["commit".into(), "--message".into(), arg.into()]
        }
        "commit-all" => {
            if arg.trim().is_empty() {
                return Err("Write what the commit does".into());
            }
            vec![
                "commit".into(),
                "--all".into(),
                "--message".into(),
                arg.into(),
            ]
        }
        "discard" => with_paths(&["restore", "--worktree", "--source=HEAD"]),
        _ => return Err(format!("unknown git operation {what}")),
    })
}

/// Clones `url` into `parent` (as a folder named after the repository).
pub fn clone_target(url: &str, parent: &Path) -> Result<PathBuf, String> {
    let u = url.trim().trim_end_matches('/');
    if u.is_empty() {
        return Err("Paste the repository's address".into());
    }
    let last = u
        .rsplit(['/', ':'])
        .next()
        .unwrap_or("")
        .trim_end_matches(".git");
    if last.is_empty() || last == "." || last == ".." {
        return Err("That address doesn't end in a repository name".into());
    }
    let target = parent.join(last);
    if target.exists() {
        return Err(format!("There's already a “{last}” here"));
    }
    Ok(target)
}

/// Changes thrown away: tracked files go back to their last commit
/// (their current content to the Trash first); untracked ones go to the
/// Trash. Returns what couldn't be done.
pub fn discard(
    root: &Path,
    paths: &[String],
    untracked: &[String],
    trash: &dyn Fn(&Path) -> Result<(), String>,
) -> Vec<(String, String)> {
    let mut failed = vec![];
    for p in untracked {
        if let Err(e) = trash(&root.join(p)) {
            failed.push((p.clone(), e));
        }
    }
    if paths.is_empty() {
        return failed;
    }
    // A copy of each as it is now goes to the Trash before git puts it back.
    let mut kept = vec![];
    for p in paths {
        let src = root.join(p);
        if src.is_file() {
            let copy = std::env::temp_dir().join(format!(
                "wr-discard-{}-{}",
                std::process::id(),
                src.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            ));
            if std::fs::copy(&src, &copy).is_ok() {
                kept.push(copy);
            }
        }
    }
    let args = match args_for("discard", "", paths) {
        Ok(a) => a,
        Err(e) => {
            failed.push((String::new(), e));
            return failed;
        }
    };
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    match run(
        root,
        &refs,
        Duration::from_secs(60),
        &AtomicBool::new(false),
        &mut |_, _| {},
    ) {
        Ok(_) => {
            for k in kept {
                let _ = trash(&k);
            }
        }
        Err(e) => {
            for k in kept {
                let _ = std::fs::remove_file(k);
            }
            failed.push((paths.join(", "), e));
        }
    }
    failed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("wr-gitops-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn g(dir: &Path, args: &[&str]) -> Result<String, String> {
        // Tests never read the user's git config.
        run(
            dir,
            args,
            Duration::from_secs(30),
            &AtomicBool::new(false),
            &mut |_, _| {},
        )
    }

    fn setup(dir: &Path) {
        std::env::set_var("GIT_CONFIG_GLOBAL", "/dev/null");
        std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
        g(dir, &["init", "--initial-branch=main"]).unwrap();
        g(dir, &["config", "user.name", "Test"]).unwrap();
        g(dir, &["config", "user.email", "test@example.com"]).unwrap();
    }

    fn op(dir: &Path, what: &str, arg: &str, paths: &[&str]) -> Result<String, String> {
        let p: Vec<String> = paths.iter().map(|s| s.to_string()).collect();
        let a = args_for(what, arg, &p)?;
        let r: Vec<&str> = a.iter().map(String::as_str).collect();
        g(dir, &r)
    }

    #[test]
    fn stage_commit_branch_and_switch() {
        let d = repo("flow");
        setup(&d);
        std::fs::write(d.join("a.txt"), "one").unwrap();
        op(&d, "stage", "", &["a.txt"]).unwrap();
        assert!(
            op(&d, "commit", " ", &[]).is_err(),
            "an empty message is refused"
        );
        op(&d, "commit", "First", &[]).unwrap();
        op(&d, "branch", "feature/x", &[]).unwrap();
        assert!(op(&d, "branch", "bad name", &[]).is_err());
        let b = branches(&d).unwrap();
        assert_eq!(
            b.iter()
                .map(|b| (b.name.as_str(), b.current))
                .collect::<Vec<_>>(),
            vec![("feature/x", true), ("main", false)]
        );
        op(&d, "switch", "main", &[]).unwrap();
        assert!(branches(&d).unwrap()[0].name == "main");
        // Commit all: changed tracked files without staging them.
        std::fs::write(d.join("a.txt"), "two").unwrap();
        op(&d, "commit-all", "Second", &[]).unwrap();
        let log = g(&d, &["log", "--format=%s"]).unwrap();
        assert_eq!(log.lines().collect::<Vec<_>>(), vec!["Second", "First"]);
        // Unstage.
        std::fs::write(d.join("b.txt"), "new").unwrap();
        op(&d, "stage", "", &["b.txt"]).unwrap();
        op(&d, "unstage", "", &["b.txt"]).unwrap();
        assert!(g(&d, &["status", "--porcelain"])
            .unwrap()
            .contains("?? b.txt"));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn clone_push_pull_and_remote_branches() {
        let base = repo("remote");
        let (origin, a, b) = (base.join("origin.git"), base.join("a"), base.join("b"));
        std::fs::create_dir_all(&origin).unwrap();
        std::env::set_var("GIT_CONFIG_GLOBAL", "/dev/null");
        std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
        g(&origin, &["init", "--bare", "--initial-branch=main"]).unwrap();
        // Cloned with progress; named after the repository.
        assert_eq!(
            clone_target(&origin.to_string_lossy(), &base).unwrap(),
            base.join("origin")
        );
        g(&base, &["clone", &origin.to_string_lossy(), "a"]).unwrap();
        g(&base, &["clone", &origin.to_string_lossy(), "b"]).unwrap();
        for r in [&a, &b] {
            g(r, &["config", "user.name", "T"]).unwrap();
            g(r, &["config", "user.email", "t@e"]).unwrap();
        }
        std::fs::write(a.join("f"), "1").unwrap();
        op(&a, "stage", "", &["f"]).unwrap();
        op(&a, "commit", "from a", &[]).unwrap();
        op(&a, "publish", "", &[]).unwrap();
        op(&a, "branch", "topic", &[]).unwrap();
        op(&a, "publish", "", &[]).unwrap();
        // b: pull, then the remote-only branch can be switched to.
        op(&b, "pull", "", &[]).unwrap();
        op(&b, "fetch", "", &[]).unwrap();
        let bb = branches(&b).unwrap();
        assert!(
            bb.iter().any(|x| x.name == "origin/topic" && x.remote),
            "{bb:?}"
        );
        op(&b, "switch-remote", "origin/topic", &[]).unwrap();
        assert!(branches(&b)
            .unwrap()
            .iter()
            .any(|x| x.name == "topic" && x.current && x.upstream == "origin/topic"));
        // Both commit: push from b is refused (with why); sync fixes it.
        op(&b, "switch", "main", &[]).unwrap();
        std::fs::write(a.join("g"), "a").unwrap();
        op(&a, "switch", "main", &[]).unwrap();
        std::fs::write(a.join("g"), "a").unwrap();
        op(&a, "stage", "", &["g"]).unwrap();
        op(&a, "commit", "a again", &[]).unwrap();
        op(&a, "push", "", &[]).unwrap();
        std::fs::write(b.join("h"), "b").unwrap();
        op(&b, "stage", "", &["h"]).unwrap();
        op(&b, "commit", "b's", &[]).unwrap();
        let e = op(&b, "push", "", &[]).unwrap_err();
        assert!(e.contains("pull (or sync) first"), "{e}");
        op(&b, "sync", "", &[]).unwrap();
        op(&b, "push", "", &[]).unwrap();
        assert!(clone_target("https://github.com/x/whiterose.git", &base)
            .unwrap()
            .ends_with("whiterose"));
        assert!(clone_target("  ", &base).is_err());
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn discarding_keeps_a_copy_in_the_trash() {
        let d = repo("discard");
        setup(&d);
        std::fs::write(d.join("a.txt"), "committed").unwrap();
        op(&d, "stage", "", &["a.txt"]).unwrap();
        op(&d, "commit", "c", &[]).unwrap();
        std::fs::write(d.join("a.txt"), "my edit").unwrap();
        std::fs::write(d.join("junk.txt"), "untracked").unwrap();
        let trashed = std::cell::RefCell::new(vec![]);
        let failed = discard(&d, &["a.txt".into()], &["junk.txt".into()], &|p| {
            trashed
                .borrow_mut()
                .push(std::fs::read_to_string(p).unwrap_or_default());
            std::fs::remove_file(p).map_err(|e| e.to_string())
        });
        assert!(failed.is_empty(), "{failed:?}");
        assert_eq!(
            std::fs::read_to_string(d.join("a.txt")).unwrap(),
            "committed"
        );
        assert!(!d.join("junk.txt").exists());
        let mut t = trashed.into_inner();
        t.sort();
        assert_eq!(t, vec!["my edit", "untracked"], "both kept in the Trash");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn progress_and_errors() {
        assert_eq!(
            progress_of("Receiving objects:  42% (420/1000), 1.2 MiB"),
            Some(("Receiving objects".into(), 42))
        );
        assert_eq!(
            progress_of("remote: Counting objects: 100% (5/5), done."),
            Some(("Counting objects".into(), 100))
        );
        assert_eq!(progress_of("fatal: nope"), None);
        let e = plain("Cloning into 'x'...\nfatal: could not read Username for 'https://github.com': terminal prompts disabled\n");
        assert!(
            e.starts_with("could not read Username") && e.contains("credential helper"),
            "{e}"
        );
        assert_eq!(parse_branches("*\0refs/heads/main\0origin/main\n \0refs/remotes/origin/HEAD\0\n \0refs/remotes/origin/main\0\n \0refs/remotes/origin/dev\0\n").iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), vec!["main", "origin/dev"]);
    }
}
