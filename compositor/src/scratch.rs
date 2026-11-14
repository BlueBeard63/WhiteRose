//! Test folders that clean up after themselves.
//!
//! The compositor's copy of `apps/scratch.rs` (the two workspaces build
//! separately); keep them the same.
//!
//! `/tmp` is usually RAM (tmpfs), so folders left behind by test runs use
//! memory and end up in swap. A `Scratch` is removed when it goes out of
//! scope, also when the test fails (panics unwind and run `Drop`). A run
//! that is killed outright can still leave one behind; the `wr-` prefix lets
//! the packaged tmpfiles rule (`packaging/files/whiterose-tmpfiles.conf`)
//! sweep those up.

#![allow(dead_code)]

use std::ffi::OsStr;
use std::ops::Deref;
use std::path::{Path, PathBuf};

/// An empty `$TMPDIR/wr-<name>-<pid>` folder, deleted with everything in it
/// when dropped. Use it like a `&Path` (`d.join(..)`, `&d`).
pub struct Scratch(PathBuf);

impl Scratch {
    /// Makes the folder, emptying any left over from an earlier run with the
    /// same process ID. `name` must be unique among tests running at once.
    pub fn new(name: &str) -> Self {
        let d = std::env::temp_dir().join(format!("wr-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        Scratch(d)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

impl Deref for Scratch {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for Scratch {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<OsStr> for Scratch {
    fn as_ref(&self) -> &OsStr {
        self.0.as_os_str()
    }
}
