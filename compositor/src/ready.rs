//! Health check markers for whiterose-launch: with WHITEROSE_READY_DIR set,
//! each part of the session leaves a file there once it's up ("compositor",
//! "shell"; the daemon leaves "daemon"). The launcher keeps a new version
//! only if all three appear in time.

/// Leaves the marker `name` (nothing without WHITEROSE_READY_DIR).
pub fn mark(name: &str) {
    let Some(dir) = std::env::var_os("WHITEROSE_READY_DIR") else { return };
    let dir = std::path::PathBuf::from(dir);
    if dir.is_dir() {
        let _ = std::fs::write(dir.join(name), b"");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn marks_only_when_asked() {
        let d = std::env::temp_dir().join(format!("wr-ready-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        // SAFETY: tests in this binary don't read this variable concurrently.
        unsafe { std::env::set_var("WHITEROSE_READY_DIR", &d) };
        super::mark("compositor");
        assert!(d.join("compositor").exists());
        unsafe { std::env::remove_var("WHITEROSE_READY_DIR") };
        super::mark("shell");
        assert!(!d.join("shell").exists());
    }
}
