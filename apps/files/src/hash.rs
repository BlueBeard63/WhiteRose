//! Checksums: MD5, SHA-1 and SHA-256 of a file in one read (Properties ›
//! Checksums; the pasted-sum check is Format.js), and checking a copy against its original (Settings › Files ›
//! Check copies).

use std::{
    fs::File,
    io::Read,
    os::unix::io::AsRawFd,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use md5::Md5;
use sha1::Sha1;
use sha2::{Digest, Sha256};

#[derive(serde::Serialize, Debug, PartialEq, Clone)]
pub struct Sums {
    pub md5: String,
    pub sha1: String,
    pub sha256: String,
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// All three sums of `path` in one read; None once `stop` is set.
/// `progress` gets the bytes read so far.
pub fn sums(
    path: &Path,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(u64),
) -> std::io::Result<Option<Sums>> {
    let mut f = File::open(path)?;
    let (mut m, mut s1, mut s2) = (Md5::new(), Sha1::new(), Sha256::new());
    let mut buf = vec![0u8; 1 << 20];
    let mut done = 0u64;
    loop {
        if stop.load(Ordering::Relaxed) {
            return Ok(None);
        }
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        m.update(&buf[..n]);
        s1.update(&buf[..n]);
        s2.update(&buf[..n]);
        done += n as u64;
        progress(done);
    }
    Ok(Some(Sums {
        md5: hex(&m.finalize()),
        sha1: hex(&s1.finalize()),
        sha256: hex(&s2.finalize()),
    }))
}

/// A running SHA-256, fed as a copy is written.
#[derive(Default)]
pub struct Running(Sha256);

impl Running {
    pub fn update(&mut self, b: &[u8]) {
        self.0.update(b)
    }
    pub fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

/// SHA-256 of what's on the drive at `path`: its cached pages are dropped
/// first (after flushing), so this reads the disk, not memory.
pub fn on_disk(path: &Path) -> std::io::Result<[u8; 32]> {
    let mut f = File::open(path)?;
    f.sync_all()?;
    // SAFETY: a valid open descriptor; advice only.
    unsafe {
        libc::posix_fadvise(f.as_raw_fd(), 0, 0, libc::POSIX_FADV_DONTNEED);
    }
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    #[test]
    fn known_sums_and_matching() {
        let d = Scratch::new("hash");
        let p = d.join("abc");
        std::fs::write(&p, "abc").unwrap();
        let mut seen = 0;
        let s = sums(&p, &AtomicBool::new(false), &mut |n| seen = n)
            .unwrap()
            .unwrap();
        // The standard test vectors for "abc".
        assert_eq!(s.md5, "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(s.sha1, "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            s.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(seen, 3);
        let mut r = Running::default();
        r.update(b"ab");
        r.update(b"c");
        assert_eq!(r.finish(), on_disk(&p).unwrap());
        assert!(sums(&p, &AtomicBool::new(true), &mut |_| {})
            .unwrap()
            .is_none());
    }
}
