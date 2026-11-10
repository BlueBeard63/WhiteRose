//! Keeping what Files remembers about you unreadable on disk (Settings ›
//! Files › Privacy › Encrypt search index and history): the search index
//! (every file name in home and on the drives), the folders you've visited
//! (Jump), and the log of what was copied, moved and deleted where.
//!
//! Off by default: on an encrypted disk (the usual setup) it adds nothing
//! and costs a little (the index is read into memory to search instead of
//! mapped). On, each file is sealed: AES-256 in counter mode, then
//! HMAC-SHA256 over all of it (encrypt-then-MAC), with a fresh random
//! nonce each time. The key is 32 random bytes kept in your keyring
//! (through whiterosed), never on disk beside the files. Without the key
//! (no keyring, daemon not running) nothing is written rather than written
//! in the clear; reading takes sealed and plain files alike, so switching
//! either way just works.

use std::{
    fs,
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

use aes::cipher::{generic_array::GenericArray, BlockEncrypt, KeyInit};
use hmac::{Hmac, Mac};
use sha2::Sha256;

const MAGIC: &[u8] = b"WRSEAL1\0";
const NONCE: usize = 16;
const TAG: usize = 32;

static ON: AtomicBool = AtomicBool::new(false);
/// None: not asked yet; Some(None): couldn't get one.
static KEY: Mutex<Option<Option<[u8; 32]>>> = Mutex::new(None);

pub fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

pub fn set_on(v: bool) {
    ON.store(v, Ordering::Relaxed);
}

/// The key, from whiterosed (made the first time).
pub fn key() -> Option<[u8; 32]> {
    let mut k = KEY.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(Some(key)) = *k {
        return Some(key);
    }
    let got = whiterose_kit::daemon::request(
        serde_json::json!({"op": "files_key"}),
        std::time::Duration::from_secs(5),
    )
    .ok()
    .and_then(|v| v.get("key").and_then(|x| x.as_str()).and_then(from_hex));
    // Not remembered when it failed: asked again next time.
    if got.is_some() {
        *k = Some(got);
    }
    got
}

#[cfg(test)]
pub fn set_key_for_tests(key: [u8; 32]) {
    *KEY.lock().unwrap() = Some(Some(key));
}

fn from_hex(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// Separate keys for encrypting and for checking, from the one key.
fn subkeys(key: &[u8; 32]) -> ([u8; 32], [u8; 32]) {
    let derive = |label: &[u8]| {
        let mut m = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("any key length");
        m.update(label);
        let out: [u8; 32] = m.finalize().into_bytes().into();
        out
    };
    (
        derive(b"whiterose files: encrypt"),
        derive(b"whiterose files: authenticate"),
    )
}

/// AES-256-CTR over `data` in place (the counter: the nonce as a 128-bit
/// big-endian number, one per 16-byte block).
fn ctr(key: &[u8; 32], nonce: &[u8; NONCE], data: &mut [u8]) {
    let cipher = aes::Aes256::new(GenericArray::from_slice(key));
    let mut counter = u128::from_be_bytes(*nonce);
    for chunk in data.chunks_mut(16) {
        let mut block = GenericArray::clone_from_slice(&counter.to_be_bytes());
        cipher.encrypt_block(&mut block);
        for (b, k) in chunk.iter_mut().zip(block.iter()) {
            *b ^= k;
        }
        counter = counter.wrapping_add(1);
    }
}

/// `plain` sealed with `key`.
pub fn seal_with(key: &[u8; 32], plain: &[u8]) -> Vec<u8> {
    let (ek, mk) = subkeys(key);
    let mut nonce = [0u8; NONCE];
    getrandom::getrandom(&mut nonce).expect("the system's random numbers");
    let mut out = Vec::with_capacity(MAGIC.len() + NONCE + plain.len() + TAG);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&nonce);
    let start = out.len();
    out.extend_from_slice(plain);
    ctr(&ek, &nonce, &mut out[start..]);
    let mut m = <Hmac<Sha256> as Mac>::new_from_slice(&mk).expect("any key length");
    m.update(&out);
    out.extend_from_slice(&m.finalize().into_bytes());
    out
}

pub fn is_sealed(data: &[u8]) -> bool {
    data.starts_with(MAGIC)
}

/// What was sealed, if `key` sealed it and nothing changed it since.
pub fn open_with(key: &[u8; 32], data: &[u8]) -> Option<Vec<u8>> {
    if !is_sealed(data) || data.len() < MAGIC.len() + NONCE + TAG {
        return None;
    }
    let (ek, mk) = subkeys(key);
    let (body, tag) = data.split_at(data.len() - TAG);
    let mut m = <Hmac<Sha256> as Mac>::new_from_slice(&mk).expect("any key length");
    m.update(body);
    // (Checked in constant time.)
    m.verify_slice(tag).ok()?;
    let nonce: [u8; NONCE] = body[MAGIC.len()..MAGIC.len() + NONCE].try_into().ok()?;
    let mut plain = body[MAGIC.len() + NONCE..].to_vec();
    ctr(&ek, &nonce, &mut plain);
    Some(plain)
}

/// A file's contents: opened if sealed (None if it can't be), as it is if not.
pub fn read(path: &Path) -> Option<Vec<u8>> {
    let data = fs::read(path).ok()?;
    if is_sealed(&data) {
        open_with(&key()?, &data)
    } else {
        Some(data)
    }
}

pub fn read_string(path: &Path) -> Option<String> {
    read(path).map(|b| String::from_utf8_lossy(&b).into_owned())
}

/// Writes `data` to `path` (only yours, replaced whole): sealed when
/// encryption is on, and not at all if it's on but there's no key.
pub fn write(path: &Path, data: &[u8]) -> Result<(), String> {
    write_as(path, data, on())
}

fn write_as(path: &Path, data: &[u8], sealed: bool) -> Result<(), String> {
    let bytes = if sealed {
        seal_with(
            &key().ok_or("no key from the keyring, so it wasn't saved")?,
            data,
        )
    } else {
        data.to_vec()
    };
    if let Some(d) = path.parent() {
        fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("sealtmp");
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp)
        .map_err(|e| e.to_string())?;
    f.write_all(&bytes).map_err(|e| e.to_string())?;
    drop(f);
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// Rewrites `path` the way the setting says now (sealed or plain), if it's
/// the other way. False if it couldn't be read.
pub fn convert(path: &Path) -> bool {
    convert_to(path, on())
}

fn convert_to(path: &Path, sealed: bool) -> bool {
    let Ok(raw) = fs::read(path) else { return true };
    if is_sealed(&raw) == sealed {
        return true;
    }
    match read(path) {
        Some(plain) => write_as(path, &plain, sealed).is_ok(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seals_and_opens() {
        let k = [7u8; 32];
        let plain = b"/home/jack/Documents/Bank statements".repeat(5);
        let s = seal_with(&k, &plain);
        assert!(is_sealed(&s));
        assert!(
            !s.windows(4).any(|w| w == b"Bank"),
            "the name isn't readable"
        );
        assert_eq!(open_with(&k, &s).unwrap(), plain);
        // A fresh nonce each time.
        assert_ne!(seal_with(&k, &plain), s);
        // Another key, or any change: refused.
        assert!(open_with(&[8u8; 32], &s).is_none());
        let mut t = s.clone();
        t[MAGIC.len() + NONCE + 3] ^= 1;
        assert!(open_with(&k, &t).is_none());
        let mut t = s.clone();
        let last = t.len() - 1;
        t[last] ^= 1;
        assert!(open_with(&k, &t).is_none());
        assert!(open_with(&k, &s[..s.len() - 1]).is_none());
        // Empty works too.
        assert_eq!(open_with(&k, &seal_with(&k, b"")).unwrap(), b"");
    }

    #[test]
    fn counter_mode_matches_the_standard() {
        // NIST SP 800-38A F.5.5 (CTR-AES256.Encrypt), first two blocks.
        let key: [u8; 32] =
            hex32("603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4");
        let nonce: [u8; 16] = hex("f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff").try_into().unwrap();
        let mut data = hex("6bc1bee22e409f96e93d7e117393172aae2d8a571e03ac9c9eb76fac45af8e51");
        ctr(&key, &nonce, &mut data);
        assert_eq!(
            data,
            hex("601ec313775789a5b7a7f504bbf3d228f443e3ca4d62b59aca84e990cacaf5c5")
        );
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }
    fn hex32(s: &str) -> [u8; 32] {
        hex(s).try_into().unwrap()
    }

    #[test]
    fn files_switch_either_way() {
        set_key_for_tests([3u8; 32]);
        let d = std::env::temp_dir().join(format!("wr-secure-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        let f = d.join("visits.json");
        // (The switch itself isn't touched: other tests run alongside.)
        write_as(&f, b"{\"/home/x\": 1}", false).unwrap();
        assert!(!is_sealed(&fs::read(&f).unwrap()));
        assert!(convert_to(&f, true));
        assert!(is_sealed(&fs::read(&f).unwrap()));
        assert_eq!(read_string(&f).unwrap(), "{\"/home/x\": 1}");
        use std::os::unix::fs::MetadataExt;
        assert_eq!(fs::metadata(&f).unwrap().mode() & 0o777, 0o600);
        assert!(convert_to(&f, false));
        assert_eq!(fs::read(&f).unwrap(), b"{\"/home/x\": 1}");
        assert_eq!(from_hex(&"ab".repeat(32)), Some([0xab; 32]));
        assert_eq!(from_hex("xyz"), None);
        fs::remove_dir_all(&d).unwrap();
    }
}
