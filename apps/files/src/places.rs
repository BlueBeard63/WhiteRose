//! The sidebar's places and the address bar's helpers:
//! - your folders (Home, Desktop, Documents, ... from ~/.config/user-dirs.dirs,
//!   so a German "Bilder" is found too);
//! - bookmarks, shared with other apps' file choosers (GTK's bookmarks file);
//! - drives: the system and anything mounted under /media, /run/media or
//!   /mnt, with their free space;
//! - breadcrumbs for a path, and folder suggestions as you type one.

use std::{
    ffi::CString,
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Place {
    pub name: String,
    pub path: String,
    pub icon: String,
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| "/".into())
}

fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".config"))
}

/// XDG user folders from user-dirs.dirs text ($HOME expanded).
pub fn user_dirs(text: &str, home: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.starts_with('#')) {
        let Some((k, v)) = line.split_once('=') else { continue };
        let Some(kind) = k.strip_prefix("XDG_").and_then(|k| k.strip_suffix("_DIR")) else { continue };
        let v = v.trim().trim_matches('"');
        let p = match v.strip_prefix("$HOME") {
            Some(rest) => home.join(rest.trim_start_matches('/')),
            None if v.starts_with('/') => PathBuf::from(v),
            None => continue,
        };
        if p != home {
            out.push((kind.to_string(), p));
        }
    }
    out
}

/// Your folders: Home first, then the user folders that exist.
pub fn folders() -> Vec<Place> {
    let h = home();
    let mut v = vec![Place { name: "Home".into(), path: h.to_string_lossy().into(), icon: "user-home".into() }];
    let text = fs::read_to_string(config_dir().join("user-dirs.dirs")).unwrap_or_default();
    let found = user_dirs(&text, &h);
    let order = ["DESKTOP", "DOCUMENTS", "DOWNLOAD", "PICTURES", "MUSIC", "VIDEOS"];
    for kind in order {
        let (path, name) = match found.iter().find(|(k, _)| k == kind) {
            Some((_, p)) => (p.clone(), p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
            None => {
                // The usual English names if user-dirs doesn't say.
                let n = match kind {
                    "DOWNLOAD" => "Downloads".to_string(),
                    k => k[..1].to_string() + &k[1..].to_lowercase(),
                };
                (h.join(&n), n)
            }
        };
        if path.is_dir() {
            let icon = match kind {
                "DESKTOP" => "user-desktop",
                "DOCUMENTS" => "folder-documents",
                "DOWNLOAD" => "folder-download",
                "PICTURES" => "folder-pictures",
                "MUSIC" => "folder-music",
                _ => "folder-videos",
            };
            v.push(Place { name, path: path.to_string_lossy().into(), icon: icon.into() });
        }
    }
    v
}

/// "file:///home/me/Code%20Stuff Label" lines from GTK's bookmarks file.
pub fn parse_bookmarks(text: &str) -> Vec<Place> {
    text.lines()
        .filter_map(|l| {
            let (uri, label) = l.split_once(' ').map(|(u, n)| (u, Some(n))).unwrap_or((l, None));
            let path = percent_decode(uri.strip_prefix("file://")?);
            let name = label.map(str::to_string).filter(|s| !s.is_empty()).unwrap_or_else(|| {
                Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone())
            });
            Some(Place { name, path, icon: "folder".into() })
        })
        .collect()
}

fn bookmarks_file() -> PathBuf {
    config_dir().join("gtk-3.0/bookmarks")
}

pub fn bookmarks() -> Vec<Place> {
    parse_bookmarks(&fs::read_to_string(bookmarks_file()).unwrap_or_default())
}

/// Adds or removes a bookmark (keeping the others, and their labels).
pub fn set_bookmark(path: &str, on: bool) -> Result<(), String> {
    let file = bookmarks_file();
    let text = fs::read_to_string(&file).unwrap_or_default();
    let uri = format!("file://{}", percent_encode(path));
    let mut lines: Vec<String> = text.lines().filter(|l| l.split(' ').next() != Some(uri.as_str())).map(String::from).collect();
    if on {
        lines.push(uri);
    }
    fs::create_dir_all(file.parent().unwrap()).map_err(|e| e.to_string())?;
    fs::write(&file, lines.join("\n") + "\n").map_err(|e| e.to_string())
}

pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn percent_encode(s: &str) -> String {
    s.bytes()
        .map(|c| match c {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => (c as char).to_string(),
            _ => format!("%{c:02X}"),
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Drive {
    pub name: String,
    pub path: String,
    pub icon: String,
    pub total: u64,
    pub free: u64,
    pub removable: bool,
}

/// Mount points from /proc/self/mountinfo text that the sidebar shows.
pub fn drive_mounts(mountinfo: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in mountinfo.lines() {
        let f: Vec<&str> = line.split(' ').collect();
        if f.len() < 10 {
            continue;
        }
        let mount = f[4].replace("\\040", " ");
        let dash = f.iter().position(|x| *x == "-").unwrap_or(0);
        let source = f.get(dash + 2).copied().unwrap_or("");
        let shown = mount.starts_with("/media/") || mount.starts_with("/run/media/") || mount.starts_with("/mnt/");
        if shown && source.starts_with("/dev/") && !out.iter().any(|(m, _)| m == &mount) {
            out.push((mount, source.to_string()));
        }
    }
    out
}

fn space(path: &str) -> (u64, u64) {
    let Ok(c) = CString::new(path) else { return (0, 0) };
    // Safety: a valid C string and a zeroed statvfs to fill.
    unsafe {
        let mut s: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(c.as_ptr(), &mut s) != 0 {
            return (0, 0);
        }
        (s.f_blocks * s.f_frsize, s.f_bavail * s.f_frsize)
    }
}

/// This computer, then mounted drives.
pub fn drives() -> Vec<Drive> {
    let (total, free) = space("/");
    let mut v = vec![Drive { name: "This computer".into(), path: "/".into(), icon: "drive-harddisk".into(), total, free, removable: false }];
    let info = fs::read_to_string("/proc/self/mountinfo").unwrap_or_default();
    for (mount, _) in drive_mounts(&info) {
        let (total, free) = space(&mount);
        let name = Path::new(&mount).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| mount.clone());
        v.push(Drive { name, path: mount, icon: "drive-removable-media".into(), total, free, removable: true });
    }
    v
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Crumb {
    pub name: String,
    pub path: String,
}

/// "/home/me/Code/app" -> Home › Code › app (or / › etc › …).
pub fn crumbs(path: &str, home: &Path) -> Vec<Crumb> {
    let home_s = home.to_string_lossy();
    let (mut out, rest, mut acc) = if path == home_s || path.starts_with(&format!("{home_s}/")) {
        (vec![Crumb { name: "Home".into(), path: home_s.to_string() }], &path[home_s.len()..], home_s.to_string())
    } else {
        (vec![Crumb { name: "/".into(), path: "/".into() }], path, String::new())
    };
    for part in rest.split('/').filter(|p| !p.is_empty()) {
        acc = format!("{acc}/{part}");
        out.push(Crumb { name: part.into(), path: acc.clone() });
    }
    out
}

/// Folders that complete what's typed ("~/Doc" -> ~/Documents), up to 8.
pub fn suggest(typed: &str, home: &Path, hidden: bool) -> Vec<String> {
    let expanded = match typed.strip_prefix('~') {
        Some(rest) => format!("{}{rest}", home.to_string_lossy()),
        None => typed.to_string(),
    };
    let (dir, prefix) = match expanded.rfind('/') {
        Some(i) => (&expanded[..=i], &expanded[i + 1..]),
        None => return Vec::new(),
    };
    let Ok(rd) = fs::read_dir(dir) else { return Vec::new() };
    let lower = prefix.to_lowercase();
    let mut v: Vec<String> = rd
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| (hidden || !n.starts_with('.') || prefix.starts_with('.')) && n.to_lowercase().starts_with(&lower))
        .map(|n| format!("{dir}{n}"))
        .collect();
    v.sort_by(|a, b| crate::listing::natural_cmp(a, b));
    v.truncate(8);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_dirs_with_home_and_absolute_paths() {
        let text = "# comment\nXDG_DESKTOP_DIR=\"$HOME/Desktop\"\nXDG_PICTURES_DIR=\"$HOME/Bilder\"\nXDG_MUSIC_DIR=\"/data/music\"\nXDG_TEMPLATES_DIR=\"$HOME/\"\n";
        let d = user_dirs(text, Path::new("/home/me"));
        assert_eq!(
            d,
            vec![
                ("DESKTOP".into(), PathBuf::from("/home/me/Desktop")),
                ("PICTURES".into(), PathBuf::from("/home/me/Bilder")),
                ("MUSIC".into(), PathBuf::from("/data/music")),
            ],
            "a folder set to $HOME itself means 'none'"
        );
    }

    #[test]
    fn bookmarks_round_trip_with_spaces_and_labels() {
        let b = parse_bookmarks("file:///home/me/Code%20Stuff\nfile:///srv/media Media server\nsftp://host/x\n");
        assert_eq!(b[0], Place { name: "Code Stuff".into(), path: "/home/me/Code Stuff".into(), icon: "folder".into() });
        assert_eq!(b[1].name, "Media server");
        assert_eq!(b.len(), 2, "only local folders");
        assert_eq!(percent_encode("/home/me/Code Stuff/ü"), "/home/me/Code%20Stuff/%C3%BC");
        assert_eq!(percent_decode("/home/me/Code%20Stuff/%C3%BC"), "/home/me/Code Stuff/ü");
    }

    #[test]
    fn drives_from_mountinfo() {
        let info = "29 1 259:2 / / rw,relatime shared:1 - ext4 /dev/nvme0n1p2 rw\n\
                    90 29 8:1 / /media/me/USB\\040STICK rw shared:2 - vfat /dev/sda1 rw\n\
                    91 29 0:50 / /run/user/1000 rw shared:3 - tmpfs tmpfs rw\n\
                    92 29 8:17 / /mnt/backup rw shared:4 - ext4 /dev/sdb1 rw\n";
        assert_eq!(
            drive_mounts(info),
            vec![("/media/me/USB STICK".into(), "/dev/sda1".into()), ("/mnt/backup".into(), "/dev/sdb1".into())]
        );
    }

    #[test]
    fn crumbs_from_home_and_root() {
        let h = Path::new("/home/me");
        let names = |p: &str| crumbs(p, h).into_iter().map(|c| c.name).collect::<Vec<_>>();
        assert_eq!(names("/home/me/Code/app"), ["Home", "Code", "app"]);
        assert_eq!(names("/home/me"), ["Home"]);
        assert_eq!(names("/etc/systemd"), ["/", "etc", "systemd"]);
        assert_eq!(names("/home/meg"), ["/", "home", "meg"], "not inside home");
        assert_eq!(crumbs("/home/me/Code", h)[1].path, "/home/me/Code");
    }

    #[test]
    fn suggestions_complete_folders() {
        let d = std::env::temp_dir().join(format!("wr-files-suggest-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        for f in ["Documents", "Downloads", "Desktop", ".config", "Music"] {
            fs::create_dir_all(d.join(f)).unwrap();
        }
        fs::write(d.join("Docfile"), "x").unwrap();
        let s = suggest(&format!("{}/Do", d.display()), Path::new("/nope"), false);
        assert_eq!(s, [format!("{}/Documents", d.display()), format!("{}/Downloads", d.display())]);
        assert_eq!(suggest("~/Mu", &d, false), [format!("{}/Music", d.display())]);
        assert_eq!(suggest("~/", &d, false).len(), 4, "hidden folders only when asked");
        assert_eq!(suggest("~/.c", &d, false), [format!("{}/.config", d.display())], "or when typed");
        fs::remove_dir_all(d).unwrap();
    }
}
