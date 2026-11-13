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
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| "/".into())
}

pub fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"))
}

/// XDG user folders from user-dirs.dirs text ($HOME expanded).
pub fn user_dirs(text: &str, home: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.starts_with('#')) {
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let Some(kind) = k.strip_prefix("XDG_").and_then(|k| k.strip_suffix("_DIR")) else {
            continue;
        };
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
    let mut v = vec![Place {
        name: "Home".into(),
        path: h.to_string_lossy().into(),
        icon: "user-home".into(),
    }];
    let text = fs::read_to_string(config_dir().join("user-dirs.dirs")).unwrap_or_default();
    let found = user_dirs(&text, &h);
    let order = [
        "DESKTOP",
        "DOCUMENTS",
        "DOWNLOAD",
        "PICTURES",
        "MUSIC",
        "VIDEOS",
    ];
    for kind in order {
        let (path, name) = match found.iter().find(|(k, _)| k == kind) {
            Some((_, p)) => (
                p.clone(),
                p.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ),
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
            v.push(Place {
                name,
                path: path.to_string_lossy().into(),
                icon: icon.into(),
            });
        }
    }
    v
}

/// "file:///home/me/Code%20Stuff Label" lines from GTK's bookmarks file.
pub fn parse_bookmarks(text: &str) -> Vec<Place> {
    text.lines()
        .filter_map(|l| {
            let (uri, label) = l
                .split_once(' ')
                .map(|(u, n)| (u, Some(n)))
                .unwrap_or((l, None));
            let path = percent_decode(uri.strip_prefix("file://")?);
            let name = label
                .map(str::to_string)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    Path::new(&path)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.clone())
                });
            Some(Place {
                name,
                path,
                icon: "folder".into(),
            })
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
    let mut lines: Vec<String> = text
        .lines()
        .filter(|l| l.split(' ').next() != Some(uri.as_str()))
        .map(String::from)
        .collect();
    if on {
        lines.push(uri);
    }
    fs::create_dir_all(file.parent().unwrap()).map_err(|e| e.to_string())?;
    fs::write(&file, lines.join("\n") + "\n").map_err(|e| e.to_string())
}

/// Moves a bookmark to position `to` (labels and the others kept).
pub fn move_bookmark(path: &str, to: usize) -> Result<(), String> {
    let file = bookmarks_file();
    let text = fs::read_to_string(&file).map_err(|e| e.to_string())?;
    let lines = reorder(&text, &format!("file://{}", percent_encode(path)), to);
    fs::write(&file, lines.join("\n") + "\n").map_err(|e| e.to_string())
}

/// The bookmark lines with `uri`'s moved to `to`.
fn reorder(text: &str, uri: &str, to: usize) -> Vec<String> {
    let mut lines: Vec<String> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(String::from)
        .collect();
    if let Some(i) = lines.iter().position(|l| l.split(' ').next() == Some(uri)) {
        let l = lines.remove(i);
        lines.insert(to.min(lines.len()), l);
    }
    lines
}

#[derive(serde::Serialize, Debug, PartialEq)]
pub struct Sub {
    pub name: String,
    pub path: String,
    /// It has folders in it (the tree shows an arrow).
    pub more: bool,
}

/// `dir` has a folder in it (stops at the first one: cheap).
pub fn has_subfolders(dir: &Path, hidden: bool) -> bool {
    fs::read_dir(dir)
        .map(|mut r| {
            r.any(|c| {
                c.map(|c| {
                    c.path().is_dir()
                        && (hidden || !c.file_name().to_string_lossy().starts_with('.'))
                })
                .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// The folders in `dir`, in natural order, for the sidebar's tree.
pub fn subfolders(dir: &Path, hidden: bool) -> Vec<Sub> {
    let Ok(rd) = fs::read_dir(dir) else {
        return vec![];
    };
    let mut v: Vec<Sub> = rd
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if !hidden && name.starts_with('.') {
                return None;
            }
            let p = e.path();
            let more = has_subfolders(&p, hidden);
            Some(Sub {
                name,
                path: p.to_string_lossy().into_owned(),
                more,
            })
        })
        .collect();
    v.sort_by(|a, b| crate::listing::natural_cmp(&a.name, &b.name));
    v
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
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                (c as char).to_string()
            }
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
    /// Its block device ("" for This computer).
    pub device: String,
    /// Shown but not mounted: click to mount (path is "" until then).
    pub mounted: bool,
    /// Plugged in (USB stick, card): can be ejected.
    pub ejectable: bool,
}

/// A filesystem on a plugged-in drive, from `lsblk -J -l -b`.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct Dev {
    pub path: String,
    pub label: Option<String>,
    pub size: Option<u64>,
    pub fstype: Option<String>,
    pub mountpoint: Option<String>,
    #[serde(default)]
    pub rm: bool,
    #[serde(default)]
    pub hotplug: bool,
    #[serde(rename = "type")]
    pub kind: String,
    pub pkname: Option<String>,
}

/// Filesystems on removable / hot-plugged drives in lsblk's JSON (swap,
/// encrypted and ones with no filesystem left out).
pub fn plugged(lsblk_json: &str) -> Vec<Dev> {
    #[derive(serde::Deserialize)]
    struct All {
        blockdevices: Vec<Dev>,
    }
    let Ok(all) = serde_json::from_str::<All>(lsblk_json) else {
        return vec![];
    };
    all.blockdevices
        .into_iter()
        .filter(|d| (d.rm || d.hotplug) && (d.kind == "part" || d.kind == "disk"))
        .filter(|d| {
            d.fstype
                .as_deref()
                .is_some_and(|f| !["swap", "crypto_LUKS", "LVM2_member", ""].contains(&f))
        })
        .collect()
}

fn lsblk() -> Vec<Dev> {
    let out = std::process::Command::new("lsblk")
        .args([
            "-J",
            "-l",
            "-b",
            "-o",
            "PATH,LABEL,SIZE,FSTYPE,MOUNTPOINT,RM,HOTPLUG,TYPE,PKNAME",
        ])
        .stderr(std::process::Stdio::null())
        .output();
    out.map(|o| plugged(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
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
        let shown = mount.starts_with("/media/")
            || mount.starts_with("/run/media/")
            || mount.starts_with("/mnt/");
        if shown && source.starts_with("/dev/") && !out.iter().any(|(m, _)| m == &mount) {
            out.push((mount, source.to_string()));
        }
    }
    out
}

fn space(path: &str) -> (u64, u64) {
    let Ok(c) = CString::new(path) else {
        return (0, 0);
    };
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
    let mut v = vec![Drive {
        name: "This computer".into(),
        path: "/".into(),
        icon: "drive-harddisk".into(),
        total,
        free,
        removable: false,
        device: String::new(),
        mounted: true,
        ejectable: false,
    }];
    let devs = lsblk();
    let info = fs::read_to_string("/proc/self/mountinfo").unwrap_or_default();
    for (mount, source) in drive_mounts(&info) {
        let (total, free) = space(&mount);
        let name = Path::new(&mount)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| mount.clone());
        v.push(Drive {
            name,
            path: mount,
            icon: "drive-removable-media".into(),
            total,
            free,
            removable: true,
            ejectable: devs.iter().any(|d| d.path == source),
            device: source,
            mounted: true,
        });
    }
    // Plugged in but not mounted: shown, to mount with a click.
    let shown: Vec<String> = v.iter().map(|x| x.device.clone()).collect();
    for d in devs
        .iter()
        .filter(|d| d.mountpoint.is_none() && !shown.contains(&d.path))
    {
        let gb = d.size.unwrap_or(0) as f64 / 1e9;
        v.push(Drive {
            name: d
                .label
                .clone()
                .filter(|l| !l.is_empty())
                .unwrap_or_else(|| format!("{:.0} GB drive", gb.max(1.0))),
            path: String::new(),
            icon: "drive-removable-media".into(),
            total: 0,
            free: 0,
            removable: true,
            device: d.path.clone(),
            mounted: false,
            ejectable: true,
        });
    }
    v
}

/// Mounts a plugged-in drive (the desktop may ask for your password):
/// where it's mounted, or what went wrong.
pub fn mount(device: &str) -> Result<String, String> {
    if !device.starts_with("/dev/") {
        return Err("not a drive".into());
    }
    let out = std::process::Command::new("udisksctl")
        .args(["mount", "-b", device])
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|_| "udisksctl isn't installed".to_string())?;
    let text = String::from_utf8_lossy(&out.stdout);
    if !out.status.success() {
        return Err(friendly_udisks(&String::from_utf8_lossy(&out.stderr)));
    }
    // "Mounted /dev/sdb1 at /media/me/USB"
    text.split(" at ")
        .nth(1)
        .map(|p| p.trim().trim_end_matches('.').to_string())
        .ok_or_else(|| text.trim().to_string())
}

/// Unmounts a drive and, when nothing else on it is mounted, powers it off
/// so it can be pulled out.
pub fn eject(device: &str) -> Result<(), String> {
    if !device.starts_with("/dev/") {
        return Err("not a drive".into());
    }
    let run = |args: &[&str]| {
        std::process::Command::new("udisksctl")
            .args(args)
            .stdin(std::process::Stdio::null())
            .output()
    };
    let out =
        run(&["unmount", "-b", device]).map_err(|_| "udisksctl isn't installed".to_string())?;
    if !out.status.success() {
        let e = String::from_utf8_lossy(&out.stderr);
        if !e.contains("not mounted") {
            return Err(friendly_udisks(&e));
        }
    }
    // The whole drive, once none of it is mounted.
    let devs = lsblk();
    let parent = devs
        .iter()
        .find(|d| d.path == device)
        .and_then(|d| d.pkname.clone())
        .map(|p| format!("/dev/{p}"));
    let whole = parent.unwrap_or_else(|| device.to_string());
    let on_it = |d: &Dev| {
        d.path == whole
            || d.pkname.as_deref().map(|p| format!("/dev/{p}")).as_deref() == Some(whole.as_str())
    };
    if !devs.iter().any(|d| d.mountpoint.is_some() && on_it(d)) {
        let _ = run(&["power-off", "-b", &whole]);
    }
    Ok(())
}

fn friendly_udisks(e: &str) -> String {
    if e.contains("busy") {
        "something is still using it (a file open, or a terminal in it)".into()
    } else if e.contains("NotAuthorized") || e.contains("not authorized") {
        "not allowed".into()
    } else {
        e.lines()
            .last()
            .unwrap_or("it didn't work")
            .trim()
            .to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Crumb {
    pub name: String,
    pub path: String,
}

/// "/home/me/Code/app" -> Home › Code › app (or / › etc › …).
pub fn crumbs(path: &str, home: &Path) -> Vec<Crumb> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(std::path::PathBuf::from);
    crumbs_in(path, home, runtime.as_deref())
}

/// A network or cloud drive's folder: its mount point (where its own
/// crumbs start) and the name people know it by. GVfs mounts are folders
/// in $XDG_RUNTIME_DIR/gvfs named for gio ("dav:host=x,ssl=true,
/// prefix=%2Fremote.php%2Fwebdav"); WhiteRose's rclone drives are in
/// $XDG_RUNTIME_DIR/whiterose/cloud/<account name>.
pub fn mount_place(path: &str, runtime: &Path) -> Option<(String, String)> {
    for (base, gvfs) in [
        (runtime.join("gvfs"), true),
        (runtime.join("whiterose/cloud"), false),
    ] {
        let base = base.to_string_lossy().into_owned();
        let Some(rest) = path.strip_prefix(&format!("{base}/")) else {
            continue;
        };
        let mount = rest.split('/').next().filter(|m| !m.is_empty())?;
        let name = if gvfs {
            gvfs_name(mount)
        } else {
            mount.to_string()
        };
        return Some((format!("{base}/{mount}"), name));
    }
    None
}

/// A GVfs mount folder's name for people: "Nextcloud" (WebDAV at
/// /remote.php/), "photos on nas" (SMB), the host (SFTP, FTP, WebDAV),
/// "Google Drive", "OneDrive".
pub fn gvfs_name(mount: &str) -> String {
    let (kind, rest) = mount.split_once(':').unwrap_or((mount, ""));
    let decode = |v: &str| {
        let b = v.as_bytes();
        let mut out = Vec::new();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'%' && i + 2 < b.len() {
                if let Some(n) = std::str::from_utf8(&b[i + 1..i + 3])
                    .ok()
                    .and_then(|h| u8::from_str_radix(h, 16).ok())
                {
                    out.push(n);
                    i += 3;
                    continue;
                }
            }
            out.push(b[i]);
            i += 1;
        }
        String::from_utf8_lossy(&out).into_owned()
    };
    let get = |k: &str| {
        rest.split(',')
            .find_map(|kv| {
                kv.split_once('=')
                    .filter(|(key, _)| *key == k)
                    .map(|(_, v)| decode(v))
            })
            .unwrap_or_default()
    };
    let host = get("host");
    match kind {
        "dav" | "davs" if get("prefix").contains("/remote.php/") => "Nextcloud".into(),
        "smb-share" if !get("share").is_empty() => format!("{} on {}", get("share"), get("server")),
        "smb-server" => get("server"),
        "google-drive" => "Google Drive".into(),
        "onedrive" => "OneDrive".into(),
        _ if !host.is_empty() => host,
        _ => mount.to_string(),
    }
}

/// Whether `path` is on a network drive (GVfs, rclone, sshfs, NFS, SMB):
/// reading it waits on a server, so Files lists it on a thread, checks it
/// for changes less often and skips thumbnails, folder sizes and git there.
/// Decided from /proc/self/mountinfo (the mount's type), never by asking
/// the drive itself, which may be the slow thing.
pub fn is_remote(path: &str) -> bool {
    std::fs::read_to_string("/proc/self/mountinfo")
        .map(|m| remote_in(&m, path))
        .unwrap_or(false)
}

/// The type of the filesystem `path` is on ("ext4", "fuse.gvfsd-fuse",
/// "vfat"), from /proc/self/mountinfo.
pub fn fs_type(path: &str) -> Option<String> {
    std::fs::read_to_string("/proc/self/mountinfo")
        .ok()
        .and_then(|m| fs_type_in(&m, path))
}

/// Whether files there have real run permissions: not on network drives
/// (GVfs shows every file as rwx), FAT/exFAT or NTFS (no permissions; all
/// shown runnable).
pub fn permissions_mean_something(path: &str) -> bool {
    !is_remote(path)
        && !fs_type(path).is_some_and(|k| {
            matches!(
                k.as_str(),
                "vfat" | "msdos" | "exfat" | "ntfs" | "ntfs3" | "fuseblk"
            )
        })
}

/// `fs_type` against a given mountinfo (tests).
pub fn fs_type_in(mountinfo: &str, path: &str) -> Option<String> {
    let unescape = |s: &str| {
        s.replace("\\040", " ")
            .replace("\\011", "\t")
            .replace("\\134", "\\")
    };
    let mut best: Option<(usize, String)> = None;
    for line in mountinfo.lines() {
        let f: Vec<&str> = line.split(' ').collect();
        let (Some(point), Some(sep)) = (f.get(4), f.iter().position(|x| *x == "-")) else {
            continue;
        };
        let Some(kind) = f.get(sep + 1) else { continue };
        let point = unescape(point);
        let inside = path == point
            || point == "/"
            || path.starts_with(&format!("{}/", point.trim_end_matches('/')));
        if inside && !best.as_ref().is_some_and(|(len, _)| point.len() < *len) {
            best = Some((point.len(), kind.to_string()));
        }
    }
    best.map(|(_, k)| k)
}

/// `is_remote` against a given mountinfo (tests).
pub fn remote_in(mountinfo: &str, path: &str) -> bool {
    let unescape = |s: &str| {
        s.replace("\\040", " ")
            .replace("\\011", "\t")
            .replace("\\134", "\\")
    };
    let mut best: Option<(usize, bool)> = None;
    for line in mountinfo.lines() {
        let f: Vec<&str> = line.split(' ').collect();
        let (Some(point), Some(sep)) = (f.get(4), f.iter().position(|x| *x == "-")) else {
            continue;
        };
        let Some(kind) = f.get(sep + 1) else { continue };
        let point = unescape(point);
        let inside = path == point
            || point == "/"
            || path.starts_with(&format!("{}/", point.trim_end_matches('/')));
        if !inside || best.is_some_and(|(len, _)| point.len() < len) {
            continue;
        }
        let remote = matches!(
            *kind,
            "nfs" | "nfs4" | "cifs" | "smb3" | "smbfs" | "davfs" | "9p"
        ) || (kind.starts_with("fuse.")
            && !matches!(*kind, "fuse.portal" | "fuse.appimagefuse" | "fuse.lxcfs"));
        best = Some((point.len(), remote));
    }
    best.is_some_and(|(_, r)| r)
}

/// `crumbs`, with $XDG_RUNTIME_DIR given (tests).
pub fn crumbs_in(path: &str, home: &Path, runtime: Option<&Path>) -> Vec<Crumb> {
    // A network or cloud drive: from its own top, by its name.
    if let Some((root, name)) = runtime.and_then(|r| mount_place(path, r)) {
        let mut out = vec![Crumb {
            name,
            path: root.clone(),
        }];
        let mut acc = root.clone();
        for part in path[root.len()..].split('/').filter(|p| !p.is_empty()) {
            acc = format!("{acc}/{part}");
            out.push(Crumb {
                name: part.into(),
                path: acc.clone(),
            });
        }
        return out;
    }
    let home_s = home.to_string_lossy();
    let (mut out, rest, mut acc) = if path == home_s || path.starts_with(&format!("{home_s}/")) {
        (
            vec![Crumb {
                name: "Home".into(),
                path: home_s.to_string(),
            }],
            &path[home_s.len()..],
            home_s.to_string(),
        )
    } else {
        (
            vec![Crumb {
                name: "/".into(),
                path: "/".into(),
            }],
            path,
            String::new(),
        )
    };
    for part in rest.split('/').filter(|p| !p.is_empty()) {
        acc = format!("{acc}/{part}");
        out.push(Crumb {
            name: part.into(),
            path: acc.clone(),
        });
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
    let Ok(rd) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let lower = prefix.to_lowercase();
    let mut v: Vec<String> = rd
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            (hidden || !n.starts_with('.') || prefix.starts_with('.'))
                && n.to_lowercase().starts_with(&lower)
        })
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
    fn reordering_bookmarks_and_listing_subfolders() {
        let text = "file:///a A\nfile:///b\n\nfile:///c Cee\n";
        assert_eq!(
            reorder(text, "file:///c", 0),
            vec!["file:///c Cee", "file:///a A", "file:///b"]
        );
        assert_eq!(
            reorder(text, "file:///a", 9),
            vec!["file:///b", "file:///c Cee", "file:///a A"]
        );
        assert_eq!(reorder(text, "file:///zzz", 0).len(), 3);
        let d = std::env::temp_dir().join(format!("wr-sub-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        for p in ["Music/x", "docs", ".hidden", "file10", "file9"] {
            fs::create_dir_all(d.join(p)).unwrap();
        }
        fs::write(d.join("note.txt"), "").unwrap();
        let s = subfolders(&d, false);
        let names: Vec<_> = s.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, vec!["docs", "file9", "file10", "Music"]);
        assert!(s.iter().find(|x| x.name == "Music").unwrap().more);
        assert!(!s.iter().find(|x| x.name == "docs").unwrap().more);
        assert_eq!(subfolders(&d, true).len(), 5);
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn bookmarks_round_trip_with_spaces_and_labels() {
        let b = parse_bookmarks(
            "file:///home/me/Code%20Stuff\nfile:///srv/media Media server\nsftp://host/x\n",
        );
        assert_eq!(
            b[0],
            Place {
                name: "Code Stuff".into(),
                path: "/home/me/Code Stuff".into(),
                icon: "folder".into()
            }
        );
        assert_eq!(b[1].name, "Media server");
        assert_eq!(b.len(), 2, "only local folders");
        assert_eq!(
            percent_encode("/home/me/Code Stuff/ü"),
            "/home/me/Code%20Stuff/%C3%BC"
        );
        assert_eq!(
            percent_decode("/home/me/Code%20Stuff/%C3%BC"),
            "/home/me/Code Stuff/ü"
        );
    }

    #[test]
    fn drives_from_mountinfo() {
        let info = "29 1 259:2 / / rw,relatime shared:1 - ext4 /dev/nvme0n1p2 rw\n\
                    90 29 8:1 / /media/me/USB\\040STICK rw shared:2 - vfat /dev/sda1 rw\n\
                    91 29 0:50 / /run/user/1000 rw shared:3 - tmpfs tmpfs rw\n\
                    92 29 8:17 / /mnt/backup rw shared:4 - ext4 /dev/sdb1 rw\n";
        assert_eq!(
            drive_mounts(info),
            vec![
                ("/media/me/USB STICK".into(), "/dev/sda1".into()),
                ("/mnt/backup".into(), "/dev/sdb1".into())
            ]
        );
    }

    #[test]
    fn knows_network_drives() {
        let m = "22 1 8:2 / / rw - ext4 /dev/sda2 rw\n\
                 40 22 0:50 / /run/user/1000/gvfs rw - fuse.gvfsd-fuse gvfsd-fuse rw\n\
                 41 22 0:51 / /mnt/nas rw - nfs4 nas:/srv rw\n\
                 42 22 0:52 / /run/user/1000/doc rw - fuse.portal portal rw\n\
                 43 22 0:53 / /media/me/My\\040Disk rw - fuseblk /dev/sdb1 rw\n\
                 44 22 0:54 / /run/user/1000/whiterose/cloud/Work rw - fuse.rclone Work: rw\n";
        assert!(remote_in(m, "/run/user/1000/gvfs/dav:host=x/Photos"));
        assert!(remote_in(m, "/mnt/nas/films"));
        assert!(remote_in(m, "/run/user/1000/whiterose/cloud/Work"));
        assert!(!remote_in(m, "/home/me/Documents"));
        assert!(
            !remote_in(m, "/run/user/1000/doc/abc"),
            "the document portal is local"
        );
        assert!(
            !remote_in(m, "/media/me/My Disk/x"),
            "an NTFS disk (fuseblk) is local"
        );
        assert!(!remote_in(m, "/mnt/nasty"), "not inside /mnt/nas");
        assert_eq!(
            fs_type_in(m, "/media/me/My Disk/a.pdf").as_deref(),
            Some("fuseblk")
        );
        assert_eq!(fs_type_in(m, "/home/me").as_deref(), Some("ext4"));
    }

    #[test]
    fn drives_by_their_names() {
        let h = Path::new("/home/me");
        let run = Path::new("/run/user/1000");
        let nc = "/run/user/1000/gvfs/dav:host=storage.bluebeard.site,ssl=true,user=jack,prefix=%2Fremote.php%2Fwebdav";
        let c = crumbs_in(&format!("{nc}/Photos/2026"), h, Some(run));
        assert_eq!(
            c.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            ["Nextcloud", "Photos", "2026"]
        );
        assert_eq!(c[0].path, nc);
        assert_eq!(c[2].path, format!("{nc}/Photos/2026"));
        assert_eq!(
            gvfs_name("smb-share:server=nas,share=photos"),
            "photos on nas"
        );
        assert_eq!(gvfs_name("sftp:host=build.local,user=jack"), "build.local");
        assert_eq!(
            gvfs_name("davs:host=files.example.org,ssl=true,prefix=%2Fdav"),
            "files.example.org"
        );
        assert_eq!(
            gvfs_name("google-drive:host=gmail.com,user=jack"),
            "Google Drive"
        );
        assert_eq!(gvfs_name("odd"), "odd");
        assert_eq!(gvfs_name("sftp:host=h%C3%A9,user=x"), "hé");
        assert_eq!(
            gvfs_name("sftp:host=%é,user=x"),
            "%é",
            "a stray % before a non-ASCII letter"
        );
        let cloud = crumbs_in("/run/user/1000/whiterose/cloud/Work/Docs", h, Some(run));
        assert_eq!(
            cloud.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            ["Work", "Docs"]
        );
        // The gvfs folder itself, and anything else: as before.
        assert_eq!(crumbs_in("/run/user/1000/gvfs", h, Some(run))[0].name, "/");
        assert_eq!(crumbs_in("/home/me/x", h, Some(run))[0].name, "Home");
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
        assert_eq!(
            s,
            [
                format!("{}/Documents", d.display()),
                format!("{}/Downloads", d.display())
            ]
        );
        assert_eq!(
            suggest("~/Mu", &d, false),
            [format!("{}/Music", d.display())]
        );
        assert_eq!(
            suggest("~/", &d, false).len(),
            4,
            "hidden folders only when asked"
        );
        assert_eq!(
            suggest("~/.c", &d, false),
            [format!("{}/.config", d.display())],
            "or when typed"
        );
        fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn plugged_drives() {
        let j = r#"{"blockdevices":[
            {"path":"/dev/nvme0n1p2","label":null,"size":500,"fstype":"ext4","mountpoint":"/","rm":false,"hotplug":false,"type":"part","pkname":"nvme0n1"},
            {"path":"/dev/sdb","label":null,"size":32000000000,"fstype":null,"mountpoint":null,"rm":true,"hotplug":true,"type":"disk","pkname":null},
            {"path":"/dev/sdb1","label":"USB","size":32000000000,"fstype":"vfat","mountpoint":null,"rm":true,"hotplug":true,"type":"part","pkname":"sdb"},
            {"path":"/dev/sdc1","label":"Secret","size":1,"fstype":"crypto_LUKS","mountpoint":null,"rm":false,"hotplug":true,"type":"part","pkname":"sdc"},
            {"path":"/dev/loop0","label":null,"size":1,"fstype":"squashfs","mountpoint":"/snap/x","rm":false,"hotplug":false,"type":"loop","pkname":null}
        ]}"#;
        let p = plugged(j);
        assert_eq!(p.len(), 1, "the USB stick's filesystem only");
        assert_eq!(
            (p[0].path.as_str(), p[0].label.as_deref()),
            ("/dev/sdb1", Some("USB"))
        );
        assert!(plugged("garbage").is_empty());
    }
}
