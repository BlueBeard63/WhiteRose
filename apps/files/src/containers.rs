//! Docker and Podman containers (the Containers place): every container
//! with its state, grouped by Compose project; start, stop, restart and
//! remove; its logs or a shell in your terminal; and its files, looked at
//! inside it (`ls` through `exec`) and copied out with `cp`.

use std::{
    path::Path,
    process::{Command, Stdio},
};

use serde::Serialize;
use serde_json::Value;

/// Which engine is installed: docker first, then podman.
pub fn engine(has: &dyn Fn(&str) -> bool) -> Option<&'static str> {
    ["docker", "podman"].into_iter().find(|e| has(e))
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Container {
    pub id: String,
    pub name: String,
    pub image: String,
    /// running | exited | paused | created | restarting | dead
    pub state: String,
    /// "Up 3 hours (healthy)", "Exited (0) 2 days ago".
    pub status: String,
    /// The Compose project it's part of ("" none), and its service.
    pub project: String,
    pub service: String,
    /// Where its compose file is ("" unknown).
    pub workdir: String,
    pub ports: String,
}

fn label(labels: &str, key: &str) -> String {
    // "a=1,b=2" (docker's ps) — values don't contain commas for these keys.
    labels
        .split(',')
        .find_map(|kv| kv.strip_prefix(&format!("{key}=")))
        .unwrap_or("")
        .to_string()
}

/// `docker ps -a --format '{{json .}}'`, one container per line (podman
/// prints a JSON array with other names).
pub fn parse(text: &str) -> Vec<Container> {
    let t = text.trim();
    let items: Vec<Value> = if t.starts_with('[') {
        serde_json::from_str(t).unwrap_or_default()
    } else {
        t.lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    };
    let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let mut out: Vec<Container> = items
        .iter()
        .map(|v| {
            // Docker: Labels "a=b,…", Names "x"; Podman: Labels {a: b}, Names ["x"].
            let (project, service, workdir) = match v.get("Labels") {
                Some(Value::String(l)) => (
                    label(l, "com.docker.compose.project"),
                    label(l, "com.docker.compose.service"),
                    label(l, "com.docker.compose.project.working_dir"),
                ),
                Some(Value::Object(m)) => {
                    let g = |k: &str| m.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                    (
                        g("com.docker.compose.project"),
                        g("com.docker.compose.service"),
                        g("com.docker.compose.project.working_dir"),
                    )
                }
                _ => Default::default(),
            };
            let name = match v.get("Names") {
                Some(Value::Array(a)) => {
                    a.first().and_then(Value::as_str).unwrap_or("").to_string()
                }
                _ => s(v, "Names"),
            };
            let state = s(v, "State").to_lowercase();
            Container {
                id: s(v, "ID")
                    .chars()
                    .take(12)
                    .collect::<String>()
                    .if_empty(s(v, "Id").chars().take(12).collect()),
                name,
                image: s(v, "Image"),
                state,
                status: s(v, "Status"),
                project,
                service,
                workdir,
                ports: match v.get("Ports") {
                    Some(Value::String(p)) => p.clone(),
                    _ => String::new(),
                },
            }
        })
        .collect();
    // Running first, then by project and name.
    out.sort_by(|a, b| {
        (a.state != "running")
            .cmp(&(b.state != "running"))
            .then(a.project.cmp(&b.project))
            .then(a.name.cmp(&b.name))
    });
    out
}

trait IfEmpty {
    fn if_empty(self, other: String) -> String;
}
impl IfEmpty for String {
    fn if_empty(self, other: String) -> String {
        if self.is_empty() {
            other
        } else {
            self
        }
    }
}

pub fn list(engine: &str) -> Result<Vec<Container>, String> {
    let o = Command::new(engine)
        .args([
            "ps",
            "--all",
            "--format",
            if engine == "podman" {
                "json"
            } else {
                "{{json .}}"
            },
        ])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    if !o.status.success() {
        let e = String::from_utf8_lossy(&o.stderr);
        return Err(if e.contains("permission denied") {
            format!("You're not allowed to use {engine} (add yourself to the {engine} group, then log in again)")
        } else if e.contains("Cannot connect") || e.contains("Is the docker daemon running") {
            format!("{engine} isn't running")
        } else {
            e.trim().to_string()
        });
    }
    Ok(parse(&String::from_utf8_lossy(&o.stdout)))
}

/// A container's id as given: hex only (it goes on a command line).
fn check_id(id: &str) -> Result<&str, String> {
    if id.len() >= 6 && id.len() <= 64 && id.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(id)
    } else {
        Err("not a container id".into())
    }
}

/// start | stop | restart | pause | unpause | remove.
pub fn act(engine: &str, what: &str, id: &str) -> Result<(), String> {
    let id = check_id(id)?;
    let args: Vec<&str> = match what {
        "start" | "stop" | "restart" | "pause" | "unpause" => vec![what, id],
        "remove" => vec!["rm", id],
        _ => return Err(format!("unknown action {what}")),
    };
    let o = Command::new(engine)
        .args(&args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    if o.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&o.stderr)
            .trim()
            .trim_start_matches("Error response from daemon: ")
            .to_string())
    }
}

/// A Compose project's command in its folder: up | down | restart | pull.
pub fn compose_args(what: &str) -> Result<Vec<&'static str>, String> {
    Ok(match what {
        "up" => vec!["compose", "up", "--detach"],
        "down" => vec!["compose", "down"],
        "restart" => vec!["compose", "restart"],
        "pull" => vec!["compose", "pull"],
        _ => return Err(format!("unknown compose action {what}")),
    })
}

pub fn compose(engine: &str, what: &str, workdir: &Path) -> Result<(), String> {
    if !workdir.is_dir() {
        return Err("Its compose folder isn't here any more".into());
    }
    let o = Command::new(engine)
        .args(compose_args(what)?)
        .current_dir(workdir)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    if o.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&o.stderr)
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("compose failed")
            .to_string())
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Entry {
    pub name: String,
    pub dir: bool,
    pub link: bool,
    pub size: u64,
}

/// `ls -la` of a folder inside a running container (busybox's too).
pub fn parse_ls(text: &str) -> Vec<Entry> {
    let mut out: Vec<Entry> = text
        .lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            if f.len() < 9 || l.starts_with("total") {
                return None;
            }
            let mode = f[0];
            // The name is everything after the 8 fields before it (spaces kept).
            let mut rest = l;
            for _ in 0..8 {
                rest = rest.trim_start();
                rest = &rest[rest.find(char::is_whitespace).unwrap_or(rest.len())..];
            }
            let mut name = rest.trim_start().to_string();
            let link = mode.starts_with('l');
            if link {
                if let Some((n, _)) = name.split_once(" -> ") {
                    name = n.to_string();
                }
            }
            (name != "." && name != "..").then(|| Entry {
                name,
                dir: mode.starts_with('d'),
                link,
                size: f[4].parse().unwrap_or(0),
            })
        })
        .collect();
    out.sort_by(|a, b| {
        b.dir
            .cmp(&a.dir)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out
}

/// What's in `path` inside container `id` (it must be running).
pub fn ls(engine: &str, id: &str, path: &str) -> Result<Vec<Entry>, String> {
    let id = check_id(id)?;
    if !path.starts_with('/') {
        return Err("not a path inside the container".into());
    }
    let o = Command::new(engine)
        .args(["exec", id, "ls", "-la", "--", path])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    if !o.status.success() {
        let e = String::from_utf8_lossy(&o.stderr);
        return Err(
            if e.contains("not running") || e.contains("is not running") {
                "It isn't running: start it to look inside".into()
            } else if e.contains("executable file not found")
                || e.contains("no such file or directory") && e.contains("exec")
            {
                "This container has no ls to look with (a minimal image)".into()
            } else {
                e.trim().lines().last().unwrap_or("").to_string()
            },
        );
    }
    Ok(parse_ls(&String::from_utf8_lossy(&o.stdout)))
}

/// Copies `path` out of container `id` into the folder `dest` (a job's
/// worth of work for big things; usually quick).
pub fn copy_out(
    engine: &str,
    id: &str,
    path: &str,
    dest: &Path,
) -> Result<std::path::PathBuf, String> {
    let id = check_id(id)?;
    let name = Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "root".into());
    let target = crate::ops::free_name(dest, &name);
    let o = Command::new(engine)
        .arg("cp")
        .arg(format!("{id}:{path}"))
        .arg(&target)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    if o.status.success() {
        Ok(target)
    } else {
        Err(String::from_utf8_lossy(&o.stderr).trim().to_string())
    }
}

/// The terminal command for a container's logs or a shell inside it.
pub fn terminal_command(engine: &str, what: &str, id: &str) -> Result<String, String> {
    let id = check_id(id)?;
    Ok(match what {
        "logs" => format!("{engine} logs --follow --tail 200 {id}"),
        // bash if it has it, else sh.
        "shell" => format!(
            "{engine} exec -it {id} sh -c 'command -v bash >/dev/null && exec bash || exec sh'"
        ),
        _ => return Err(format!("unknown {what}")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_docker_and_podman_lists() {
        let docker = r#"{"ID":"0c171ff3df4d","Image":"postgres:17-alpine","Labels":"com.docker.compose.project=infisical,com.docker.compose.service=db,com.docker.compose.project.working_dir=/home/me/infisical","Names":"infisical-db-1","State":"running","Status":"Up 2 hours (healthy)","Ports":"5432/tcp"}
{"ID":"46821626eb41","Image":"redis","Labels":"","Names":"cache","State":"exited","Status":"Exited (0) 2 days ago","Ports":""}
"#;
        let c = parse(docker);
        assert_eq!(c.len(), 2);
        assert_eq!(
            (
                c[0].name.as_str(),
                c[0].project.as_str(),
                c[0].service.as_str(),
                c[0].workdir.as_str()
            ),
            ("infisical-db-1", "infisical", "db", "/home/me/infisical")
        );
        assert_eq!(c[1].state, "exited");
        let podman = r#"[{"Id":"abcdef0123456789","Image":"nginx","Names":["web"],"State":"running","Status":"Up 1 minute","Labels":{"com.docker.compose.project":"site"}}]"#;
        let p = parse(podman);
        assert_eq!(
            (p[0].id.as_str(), p[0].name.as_str(), p[0].project.as_str()),
            ("abcdef012345", "web", "site")
        );
        assert_eq!(engine(&|e| e == "podman"), Some("podman"));
        assert_eq!(engine(&|_| false), None);
    }

    #[test]
    fn reads_ls_output() {
        let ls = "total 12\ndrwxr-xr-x    1 root     root          4096 Nov 10 12:00 .\ndrwxr-xr-x    1 root     root          4096 Nov 10 12:00 ..\n-rw-r--r--    1 root     root           120 Nov 10 12:00 my notes.txt\nlrwxrwxrwx    1 root     root             7 Nov 10 12:00 bin -> usr/bin\ndrwxr-xr-x    2 root     root          4096 Nov 10 12:00 etc\n";
        let e = parse_ls(ls);
        assert_eq!(
            e.iter()
                .map(|x| (x.name.as_str(), x.dir, x.link))
                .collect::<Vec<_>>(),
            vec![
                ("etc", true, false),
                ("bin", false, true),
                ("my notes.txt", false, false)
            ]
        );
        assert_eq!(e[2].size, 120);
    }

    #[test]
    fn ids_and_commands_are_checked() {
        assert!(act("docker", "start", "abc; rm -rf /").is_err());
        assert!(act("docker", "explode", "0c171ff3df4d").is_err());
        assert!(ls("docker", "0c171ff3df4d", "etc").is_err());
        assert_eq!(
            terminal_command("podman", "logs", "0c171ff3df4d").unwrap(),
            "podman logs --follow --tail 200 0c171ff3df4d"
        );
        assert!(terminal_command("docker", "shell", "x y").is_err());
        assert!(compose_args("destroy").is_err());
    }
}
