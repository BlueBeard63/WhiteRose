//! Opening a folder in a terminal or a code editor: which are installed,
//! how each is told where to start, and the git repository a folder is in.

use std::{
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct App {
    /// The command (also the setting's value).
    pub id: &'static str,
    pub name: &'static str,
}

/// Terminals, in the order one is picked when none is chosen.
pub const TERMINALS: &[App] = &[
    App {
        id: "ghostty",
        name: "Ghostty",
    },
    App {
        id: "ptyxis",
        name: "Ptyxis",
    },
    App {
        id: "kgx",
        name: "Console",
    },
    App {
        id: "gnome-terminal",
        name: "GNOME Terminal",
    },
    App {
        id: "konsole",
        name: "Konsole",
    },
    App {
        id: "kitty",
        name: "kitty",
    },
    App {
        id: "alacritty",
        name: "Alacritty",
    },
    App {
        id: "foot",
        name: "foot",
    },
    App {
        id: "wezterm",
        name: "WezTerm",
    },
    App {
        id: "xterm",
        name: "XTerm",
    },
];

/// Code editors, likewise (WhiteRose Notepad last: always there).
pub const EDITORS: &[App] = &[
    App {
        id: "code",
        name: "VS Code",
    },
    App {
        id: "codium",
        name: "VSCodium",
    },
    App {
        id: "zed",
        name: "Zed",
    },
    App {
        id: "subl",
        name: "Sublime Text",
    },
    App {
        id: "gnome-text-editor",
        name: "Text Editor",
    },
    App {
        id: "kate",
        name: "Kate",
    },
    App {
        id: "whiterose-notepad",
        name: "Notepad",
    },
];

/// `name` on $PATH.
pub fn installed(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| is_exe(&d.join(name))))
        .unwrap_or(false)
}

fn is_exe(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Those of `list` installed here.
pub fn available(list: &[App], has: impl Fn(&str) -> bool) -> Vec<App> {
    list.iter().filter(|a| has(a.id)).cloned().collect()
}

/// The one to use: the chosen one if it's there, else the first installed.
pub fn pick<'a>(list: &'a [App], chosen: &str, has: impl Fn(&str) -> bool) -> Option<&'a App> {
    list.iter()
        .find(|a| a.id == chosen && has(a.id))
        .or_else(|| list.iter().find(|a| has(a.id)))
}

/// How `terminal` is started in `dir`.
pub fn terminal_args(terminal: &str, dir: &str) -> Vec<String> {
    let d = dir.to_string();
    match terminal {
        "ghostty" | "foot" | "gnome-terminal" => vec![format!("--working-directory={d}")],
        "wezterm" => vec!["start".into(), "--cwd".into(), d],
        "ptyxis" => vec!["--new-window".into(), "--working-directory".into(), d],
        "kgx" | "alacritty" => vec!["--working-directory".into(), d],
        "konsole" => vec!["--workdir".into(), d],
        "kitty" => vec!["--directory".into(), d],
        _ => vec![],
    }
}

/// Starts `cmd args` in `dir`, on its own (it outlives Files).
pub fn spawn(cmd: &str, args: &[String], dir: &str) -> Result<(), String> {
    let mut c = Command::new(cmd);
    c.args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // SAFETY: setsid in the child before exec only.
    unsafe {
        c.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    c.spawn().map(|_| ()).map_err(|e| format!("{cmd}: {e}"))
}

/// The git repository `p` is in (the folder holding `.git`), if any.
pub fn git_root(p: &Path) -> Option<PathBuf> {
    let mut d = Some(p);
    while let Some(dir) = d {
        if dir.join(".git").exists() {
            return Some(dir.to_path_buf());
        }
        d = dir.parent();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picking() {
        let has = |n: &str| ["ptyxis", "xterm"].contains(&n);
        assert_eq!(
            pick(TERMINALS, "", has).map(|a| a.id),
            Some("ptyxis"),
            "the first installed"
        );
        assert_eq!(
            pick(TERMINALS, "xterm", has).map(|a| a.id),
            Some("xterm"),
            "the chosen one"
        );
        assert_eq!(
            pick(TERMINALS, "ghostty", has).map(|a| a.id),
            Some("ptyxis"),
            "chosen but gone: the first installed"
        );
        assert_eq!(pick(TERMINALS, "", |_| false), None);
        assert_eq!(
            available(EDITORS, |n| n == "zed" || n == "whiterose-notepad").len(),
            2
        );
    }

    #[test]
    fn starting_in_a_folder() {
        assert_eq!(
            terminal_args("ghostty", "/a b"),
            vec!["--working-directory=/a b"]
        );
        assert_eq!(
            terminal_args("ptyxis", "/x"),
            vec!["--new-window", "--working-directory", "/x"]
        );
        assert_eq!(terminal_args("konsole", "/x"), vec!["--workdir", "/x"]);
        assert_eq!(terminal_args("wezterm", "/x"), vec!["start", "--cwd", "/x"]);
        assert!(
            terminal_args("xterm", "/x").is_empty(),
            "started in the folder"
        );
    }

    #[test]
    fn repositories() {
        let d = std::env::temp_dir().join(format!("wr-git-{}", std::process::id()));
        std::fs::create_dir_all(d.join("repo/.git")).unwrap();
        std::fs::create_dir_all(d.join("repo/src/deep")).unwrap();
        assert_eq!(git_root(&d.join("repo/src/deep")), Some(d.join("repo")));
        assert_eq!(git_root(&d.join("repo")), Some(d.join("repo")));
        // (The temp folder itself isn't in a repository.)
        assert_eq!(git_root(&d), None);
    }
}
