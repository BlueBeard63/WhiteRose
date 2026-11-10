//! Files never runs code by itself. Opening a program, a script or a
//! launcher asks first; if it's somewhere files from outside land
//! (Downloads, the Desktop, a USB stick, temporary folders, the Trash, or
//! out of an archive),
//! running it takes saying you trust it, and launchers from there don't run
//! at all.

use std::{
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Path,
};

use serde::Serialize;

#[derive(Serialize, Debug, PartialEq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Opens normally (a document, a picture...).
    None,
    /// A program: has the run permission, or is one (ELF, AppImage).
    Program,
    /// A script with the run permission.
    Script,
    /// A .desktop launcher (it names a command to run).
    Launcher,
}

pub fn kind_of(path: &Path, mime: &str) -> Kind {
    let Ok(m) = std::fs::metadata(path) else {
        return Kind::None;
    };
    if !m.is_file() {
        return Kind::None;
    }
    if mime == "application/x-desktop" || path.extension().is_some_and(|e| e == "desktop") {
        return Kind::Launcher;
    }
    let exec = m.permissions().mode() & 0o111 != 0;
    let program_type = [
        "application/x-executable",
        "application/x-pie-executable",
        "application/x-sharedlib",
        "application/vnd.appimage",
        "application/x-appimage",
        "application/x-ms-dos-executable",
        "application/x-msdownload",
    ]
    .contains(&mime);
    if program_type {
        return Kind::Program;
    }
    if exec {
        let script = mime.starts_with("text/")
            || [
                "application/x-shellscript",
                "application/x-perl",
                "application/x-ruby",
                "application/javascript",
            ]
            .contains(&mime);
        return if script { Kind::Script } else { Kind::Program };
    }
    Kind::None
}

/// Why `path` is somewhere untrusted ("" if it isn't).
pub fn untrusted(
    path: &Path,
    home: &Path,
    downloads: Option<&Path>,
    removable: &[String],
) -> String {
    let under = |p: &Path| path.starts_with(p);
    if let Some(d) = downloads {
        if under(d) {
            return "It's in Downloads, where files from the internet land.".into();
        }
    }
    if under(&home.join("Desktop")) {
        return "It's on the Desktop, where downloads and attachments often land.".into();
    }
    if under(&crate::trash::home_trash()) || path.to_string_lossy().contains("/.Trash-") {
        return "It's in the Trash.".into();
    }
    // Taken out of an archive to open (archive.rs: cached_file).
    if path.components().collect::<Vec<_>>().windows(3).any(|w| {
        w[0].as_os_str() == "whiterose"
            && w[1].as_os_str() == "files"
            && w[2].as_os_str() == "opened"
    }) {
        return "It came out of an archive.".into();
    }
    for t in ["/tmp", "/var/tmp", "/dev/shm"] {
        if under(Path::new(t)) {
            return "It's in a temporary folder anyone on this computer can write to.".into();
        }
    }
    if removable.iter().any(|r| under(Path::new(r)))
        || ["/media", "/run/media", "/mnt"]
            .iter()
            .any(|r| under(Path::new(r)))
    {
        return "It's on a removable drive.".into();
    }
    // SAFETY: getuid can't fail.
    let me = unsafe { libc::getuid() };
    if std::fs::metadata(path)
        .map(|m| m.uid() != me && m.uid() != 0)
        .unwrap_or(false)
    {
        return "It belongs to someone else.".into();
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_runs() {
        let d = std::env::temp_dir().join(format!("wr-run-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let w = |n: &str, mode: u32| {
            let p = d.join(n);
            std::fs::write(&p, "#!/bin/sh\n").unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode)).unwrap();
            p
        };
        assert_eq!(kind_of(&w("notes.txt", 0o644), "text/plain"), Kind::None);
        assert_eq!(
            kind_of(&w("setup.sh", 0o755), "application/x-shellscript"),
            Kind::Script
        );
        assert_eq!(
            kind_of(&w("setup2.sh", 0o644), "application/x-shellscript"),
            Kind::None,
            "no run permission: opens as text"
        );
        assert_eq!(
            kind_of(&w("tool", 0o755), "application/octet-stream"),
            Kind::Program
        );
        assert_eq!(
            kind_of(&w("App.AppImage", 0o644), "application/vnd.appimage"),
            Kind::Program
        );
        assert_eq!(
            kind_of(&w("evil.desktop", 0o644), "application/x-desktop"),
            Kind::Launcher
        );
        assert_eq!(kind_of(&d, "inode/directory"), Kind::None);
    }

    #[test]
    fn where_it_is() {
        let home = Path::new("/home/me");
        let dl = Path::new("/home/me/Downloads");
        let r = |p: &str| untrusted(Path::new(p), home, Some(dl), &["/run/media/me/USB".into()]);
        assert!(r("/home/me/Downloads/x/setup.sh").contains("Downloads"));
        assert!(r("/home/me/Desktop/a").contains("Desktop"));
        assert!(r("/tmp/a").contains("temporary"));
        assert!(r("/run/media/me/USB/a").contains("removable"));
        assert!(r("/media/stick/a").contains("removable"));
        assert_eq!(r("/home/me/Projects/build.sh"), "");
        assert!(r("/home/me/.cache/whiterose/files/opened/0123abcd/setup.sh").contains("archive"));
    }
}
