//! What kind of project a folder is (Rust, Node, Go, Python, Make, just,
//! CMake, Compose…) and the scripts it has, for the details panel's
//! Scripts list. A script runs in your terminal, in the folder, and the
//! terminal stays open afterwards to read what it said.

use std::path::Path;

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Script {
    /// Shown, e.g. "test" or "build".
    pub name: String,
    /// What runs, e.g. "cargo test" or "npm run build".
    pub command: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Project {
    /// "Rust", "Node", ...
    pub kind: String,
    /// The file it's known by.
    pub file: String,
    pub scripts: Vec<Script>,
}

fn script(name: &str, command: &str) -> Script {
    Script {
        name: name.into(),
        command: command.into(),
    }
}

fn read(dir: &Path, f: &str) -> Option<String> {
    let p = dir.join(f);
    // A project file is small; anything huge isn't read.
    if std::fs::metadata(&p).ok()?.len() > 1 << 20 {
        return None;
    }
    std::fs::read_to_string(p).ok()
}

/// A shell word, quoted if it needs to be.
pub fn quote(s: &str) -> String {
    if !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./:=@+".contains(c))
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

/// Make (and just) targets: "name:" at the start of a line, not a
/// variable (`x := 1`), a pattern (`%.o:`) or a special one (`.PHONY:`).
fn targets(text: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for l in text.lines() {
        if l.starts_with([' ', '\t', '#', '.']) {
            continue;
        }
        let Some((head, rest)) = l.split_once(':') else {
            continue;
        };
        if rest.starts_with('=') || head.contains(['=', '%', '$', '(']) {
            continue;
        }
        // "build test: deps" makes two; just's "name arg:" makes one.
        for name in head.split_whitespace().take(1) {
            let ok = name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_/".contains(c));
            if ok && !out.iter().any(|o| o == name) {
                out.push(name.to_string());
            }
        }
    }
    out
}

/// The package manager a Node project uses (from its lock file).
fn node_runner(dir: &Path) -> &'static str {
    if dir.join("pnpm-lock.yaml").exists() {
        "pnpm"
    } else if dir.join("yarn.lock").exists() {
        "yarn"
    } else if dir.join("bun.lockb").exists() || dir.join("bun.lock").exists() {
        "bun run"
    } else {
        "npm run"
    }
}

/// Every kind of project `dir` is (a folder can be several: a Go service
/// with a Makefile and a compose file).
pub fn detect(dir: &Path) -> Vec<Project> {
    let mut out = vec![];
    let mut add = |kind: &str, file: &str, scripts: Vec<Script>| {
        out.push(Project {
            kind: kind.into(),
            file: file.into(),
            scripts,
        })
    };

    if dir.join("Cargo.toml").is_file() {
        add(
            "Rust",
            "Cargo.toml",
            vec![
                script("build", "cargo build"),
                script("run", "cargo run"),
                script("test", "cargo test"),
                script("check", "cargo check"),
                script("clippy", "cargo clippy"),
            ],
        );
    }
    if let Some(text) = read(dir, "package.json") {
        let runner = node_runner(dir);
        let mut s = vec![];
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(obj) = v.get("scripts").and_then(|x| x.as_object()) {
                for k in obj.keys() {
                    s.push(script(k, &format!("{runner} {}", quote(k))));
                }
            }
        }
        let install = match runner {
            "pnpm" => "pnpm install",
            "yarn" => "yarn install",
            "bun run" => "bun install",
            _ => "npm install",
        };
        s.push(script("install", install));
        add("Node", "package.json", s);
    }
    if dir.join("go.mod").is_file() {
        add(
            "Go",
            "go.mod",
            vec![
                script("build", "go build ./..."),
                script("run", "go run ."),
                script("test", "go test ./..."),
                script("vet", "go vet ./..."),
            ],
        );
    }
    if let Some(text) = read(dir, "pyproject.toml") {
        let uv = dir.join("uv.lock").exists();
        let run = if uv { "uv run " } else { "" };
        let mut s = vec![];
        // [project.scripts] name = "module:func"
        let mut in_scripts = false;
        for l in text.lines() {
            let t = l.trim();
            if t.starts_with('[') {
                in_scripts = t == "[project.scripts]" || t == "[tool.poetry.scripts]";
                continue;
            }
            if in_scripts {
                if let Some((k, _)) = t.split_once('=') {
                    let k = k.trim().trim_matches('"');
                    if !k.is_empty() {
                        s.push(script(k, &format!("{run}{}", quote(k))));
                    }
                }
            }
        }
        s.push(script("test", &format!("{run}python -m pytest")));
        if uv {
            s.push(script("sync", "uv sync"));
        }
        add("Python", "pyproject.toml", s);
    }
    for f in ["Makefile", "makefile", "GNUmakefile"] {
        if let Some(text) = read(dir, f) {
            let t = targets(&text);
            if !t.is_empty() {
                add(
                    "Make",
                    f,
                    t.iter()
                        .map(|n| script(n, &format!("make {}", quote(n))))
                        .collect(),
                );
            }
            break;
        }
    }
    for f in ["justfile", "Justfile", ".justfile"] {
        if let Some(text) = read(dir, f) {
            let t = targets(&text);
            if !t.is_empty() {
                add(
                    "just",
                    f,
                    t.iter()
                        .map(|n| script(n, &format!("just {}", quote(n))))
                        .collect(),
                );
            }
            break;
        }
    }
    if dir.join("CMakeLists.txt").is_file() {
        add(
            "CMake",
            "CMakeLists.txt",
            vec![
                script("configure", "cmake -B build"),
                script("build", "cmake --build build"),
                script("test", "ctest --test-dir build"),
            ],
        );
    }
    for f in [
        "compose.yaml",
        "compose.yml",
        "docker-compose.yml",
        "docker-compose.yaml",
    ] {
        if dir.join(f).is_file() {
            add(
                "Compose",
                f,
                vec![
                    script("up", "docker compose up -d"),
                    script("down", "docker compose down"),
                    script("logs", "docker compose logs -f"),
                    script("ps", "docker compose ps"),
                    script("pull", "docker compose pull"),
                ],
            );
            break;
        }
    }
    out
}

/// The shell command a terminal runs for a script: in `dir`, then says
/// how it ended and waits, so its output can be read.
pub fn wrapped(command: &str) -> String {
    format!(
        "( {command} ); s=$?; echo; if [ $s -eq 0 ]; then echo '— done —'; else echo \"— stopped with code $s —\"; fi; printf 'Press Enter to close'; read _"
    )
}

/// How `terminal` runs `sh -c <cmd>` in `dir` (its own way of being told).
pub fn terminal_run_args(terminal: &str, dir: &str, cmd: &str) -> Vec<String> {
    let mut a = crate::launch::terminal_args(terminal, dir);
    let sh = ["sh".to_string(), "-c".to_string(), cmd.to_string()];
    match terminal {
        "gnome-terminal" | "kgx" | "ptyxis" | "wezterm" => a.push("--".into()),
        "ghostty" | "konsole" | "alacritty" | "xterm" => a.push("-e".into()),
        // kitty and foot take the command as it is.
        _ => {}
    }
    a.extend(sh);
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("wr-proj-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        for (f, body) in files {
            std::fs::write(d.join(f), body).unwrap();
        }
        d
    }

    fn names(p: &Project) -> Vec<&str> {
        p.scripts.iter().map(|s| s.name.as_str()).collect()
    }

    #[test]
    fn finds_projects_and_scripts() {
        let d = dir(
            "many",
            &[
                ("package.json", r#"{"scripts": {"dev": "vite", "build": "vite build", "lint it": "eslint ."}}"#),
                ("pnpm-lock.yaml", ""),
                ("Makefile", "CC := gcc\n.PHONY: all test\nall: build\nbuild test: deps\n\tgcc x\n%.o: %.c\n\t$(CC)\n# comment:\ninstall:\n"),
                ("go.mod", "module x"),
                ("compose.yaml", "services: {}"),
                ("pyproject.toml", "[project]\nname='x'\n[project.scripts]\nserve = \"x:main\"\n[tool.x]\na = 1\n"),
                ("uv.lock", ""),
            ],
        );
        let p = detect(&d);
        let kinds: Vec<&str> = p.iter().map(|x| x.kind.as_str()).collect();
        assert_eq!(kinds, ["Node", "Go", "Python", "Make", "Compose"]);
        let node = &p[0];
        // In the order the file has them.
        assert_eq!(names(node), ["dev", "build", "lint it", "install"]);
        assert_eq!(node.scripts[2].command, "pnpm 'lint it'");
        assert_eq!(node.scripts[3].command, "pnpm install");
        assert_eq!(names(&p[2]), ["serve", "test", "sync"]);
        assert_eq!(p[2].scripts[0].command, "uv run serve");
        assert_eq!(names(&p[3]), ["all", "build", "install"]);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn nothing_for_a_plain_folder() {
        let d = dir("plain", &[("notes.txt", "x")]);
        assert!(detect(&d).is_empty());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn terminals_run_the_command() {
        assert_eq!(
            terminal_run_args("ghostty", "/p", "ls"),
            ["--working-directory=/p", "-e", "sh", "-c", "ls"]
        );
        assert_eq!(
            terminal_run_args("ptyxis", "/p", "ls"),
            [
                "--new-window",
                "--working-directory",
                "/p",
                "--",
                "sh",
                "-c",
                "ls"
            ]
        );
        assert_eq!(
            terminal_run_args("kitty", "/p", "ls"),
            ["--directory", "/p", "sh", "-c", "ls"]
        );
        assert_eq!(quote("a b's"), "'a b'\\''s'");
        // The wrapped command reports how it ended and waits.
        let out = std::process::Command::new("sh")
            .args(["-c", &wrapped("exit 3")])
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        assert!(String::from_utf8_lossy(&out.stdout).contains("stopped with code 3"));
    }
}
