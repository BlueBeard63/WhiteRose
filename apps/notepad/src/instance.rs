//! One Notepad per desktop session. Later launches hand their file paths to
//! the first process instead of restoring the same session in another window.
//! A kernel lock serializes startup and makes a stale socket safe to replace.

use std::{
    collections::hash_map::DefaultHasher,
    fs::{self, File, OpenOptions},
    hash::{Hash, Hasher},
    io::{self, BufRead, BufReader, Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{DirBuilderExt, FileTypeExt, OpenOptionsExt, PermissionsExt},
            net::{UnixListener, UnixStream},
        },
    },
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant},
};

static PRIMARY: Mutex<Option<Primary>> = Mutex::new(None);
const TIMEOUT: Duration = Duration::from_secs(3);
const MAX_REQUEST: u64 = 1024 * 1024;

pub struct Primary {
    listener: UnixListener,
    // Must stay open for as long as the listener exists.
    _lock: File,
    path: PathBuf,
    pub arguments: Vec<String>,
}

pub enum Launch {
    Primary(Primary),
    Forwarded,
}

pub fn socket_path() -> PathBuf {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::session::dir().join("run"));
    let mut hash = DefaultHasher::new();
    // The display identifies the desktop even if a launcher hasn't inherited
    // WhiteRose's optional IPC environment variables.
    if let Some(display) = std::env::var_os("WAYLAND_DISPLAY") {
        runtime.join(display).hash(&mut hash);
    } else {
        std::env::var_os("DISPLAY").hash(&mut hash);
        std::env::var_os("DBUS_SESSION_BUS_ADDRESS").hash(&mut hash);
    }
    crate::session::dir().hash(&mut hash);
    runtime
        .join("whiterose-notepad")
        .join(format!("{:016x}.sock", hash.finish()))
}

/// Files are normalized in the sender's working directory, not the original
/// Notepad's. Existing symlinks resolve to the tab that's already open.
pub fn normalize(path: &Path, cwd: &Path) -> PathBuf {
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        cwd.join(path)
    };
    path.canonicalize().unwrap_or(path)
}

pub fn argument_path(argument: &str, cwd: &Path) -> PathBuf {
    let local = if argument.starts_with("file:") {
        cxx_qt_lib::QUrl::from(argument)
            .to_local_file()
            .map(|path| path.to_string())
            .unwrap_or_else(|| argument.to_owned())
    } else {
        argument.to_owned()
    };
    normalize(Path::new(&local), cwd)
}

pub fn launch(path: &Path, arguments: Vec<String>) -> io::Result<Launch> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::other("no runtime directory"))?;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path.with_extension("lock"))?;
    // SAFETY: `lock` owns a live fd. flock doesn't retain a Rust pointer, and
    // closing this file releases the lock even if Notepad crashes.
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::WouldBlock {
            return Err(error);
        }
        forward(path, &arguments)?;
        return Ok(Launch::Forwarded);
    }
    // Only the lock holder may clean up a crashed predecessor's socket.
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_socket() => fs::remove_file(path)?,
        Ok(_) => return Err(io::Error::other("Notepad's socket path is not a socket")),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let listener = UnixListener::bind(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(Launch::Primary(Primary {
        listener,
        _lock: lock,
        path: path.to_owned(),
        arguments,
    }))
}

fn forward(path: &Path, arguments: &[String]) -> io::Result<()> {
    let deadline = Instant::now() + TIMEOUT;
    // The first process may hold the lock just before it binds the listener.
    let mut stream = loop {
        match UnixStream::connect(path) {
            Ok(s) => break s,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                ) && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => return Err(e),
        }
    };
    stream.set_read_timeout(Some(TIMEOUT))?;
    stream.set_write_timeout(Some(TIMEOUT))?;
    let request = serde_json::to_vec(arguments)?;
    if request.len() as u64 >= MAX_REQUEST {
        return Err(io::Error::other("too many file names"));
    }
    stream.write_all(&request)?;
    stream.write_all(b"\n")?;
    let mut ack = [0];
    stream.read_exact(&mut ack)?;
    if ack != [1] {
        return Err(io::Error::other("Notepad couldn't accept the files"));
    }
    Ok(())
}

impl Primary {
    fn receive(&self, open: &mut impl FnMut(Vec<String>) -> bool) -> io::Result<()> {
        let (mut stream, _) = self.listener.accept()?;
        stream.set_read_timeout(Some(TIMEOUT))?;
        stream.set_write_timeout(Some(TIMEOUT))?;
        let mut line = String::new();
        BufReader::new((&stream).take(MAX_REQUEST)).read_line(&mut line)?;
        if !line.ends_with('\n') {
            return Err(io::Error::other("incomplete file-open request"));
        }
        let paths: Vec<String> = serde_json::from_str(&line)?;
        stream.write_all(&[u8::from(open(paths))])
    }

    pub fn serve(self, mut open: impl FnMut(Vec<String>) -> bool + Send + 'static) {
        std::thread::spawn(move || loop {
            if let Err(e) = self.receive(&mut open) {
                eprintln!("Notepad: file-open request: {e}");
            }
        });
    }
}

impl Drop for Primary {
    fn drop(&mut self) {
        // The lock is still held here; another instance cannot bind yet.
        let _ = fs::remove_file(&self.path);
    }
}

pub fn install(primary: Primary) {
    *PRIMARY.lock().unwrap() = Some(primary);
}

pub fn take() -> Option<Primary> {
    PRIMARY.lock().unwrap().take()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    #[test]
    fn later_launches_forward_files_and_an_empty_launch_activates() {
        let dir = Scratch::new("notepad-instance");
        let path = dir.join("app.sock");
        let Launch::Primary(primary) = launch(&path, vec!["/first.txt".into()]).unwrap() else {
            panic!("first launch should own the instance");
        };
        assert_eq!(primary.arguments, vec!["/first.txt"]);
        let client = std::thread::spawn(move || {
            assert!(matches!(
                launch(&path, vec!["/a file.txt".into(), "/second.txt".into()]).unwrap(),
                Launch::Forwarded
            ));
            assert!(matches!(launch(&path, vec![]).unwrap(), Launch::Forwarded));
        });
        let mut requests = Vec::new();
        for _ in 0..2 {
            primary
                .receive(&mut |paths| {
                    requests.push(paths);
                    true
                })
                .unwrap();
        }
        client.join().unwrap();
        assert_eq!(
            requests,
            vec![
                vec!["/a file.txt".to_string(), "/second.txt".into()],
                vec![]
            ]
        );
    }

    #[test]
    fn a_stale_socket_is_replaced_after_a_crash_and_unlock() {
        let dir = Scratch::new("notepad-stale");
        let path = dir.join("app.sock");
        drop(UnixListener::bind(&path).unwrap()); // a dead process leaves this
        let Launch::Primary(primary) = launch(&path, vec![]).unwrap() else {
            panic!("stale socket shouldn't prevent startup");
        };
        drop(primary);
        assert!(!path.exists());
        assert!(matches!(launch(&path, vec![]).unwrap(), Launch::Primary(_)));
    }

    #[test]
    fn non_socket_files_are_never_removed() {
        let dir = Scratch::new("notepad-not-socket");
        let path = dir.join("app.sock");
        fs::write(&path, "keep me").unwrap();
        assert!(launch(&path, vec![]).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "keep me");
    }

    #[test]
    fn relative_and_symlink_paths_have_one_identity() {
        let dir = Scratch::new("notepad-paths");
        let file = dir.join("a.txt");
        fs::write(&file, "a").unwrap();
        std::os::unix::fs::symlink(&file, dir.join("link.txt")).unwrap();
        assert_eq!(normalize(Path::new("a.txt"), &dir), file);
        assert_eq!(normalize(Path::new("link.txt"), &dir), file);
    }

    #[test]
    fn file_urls_decode_but_percent_signs_in_plain_paths_are_literal() {
        let dir = Scratch::new("notepad-url");
        let file = dir.join("a b.txt");
        fs::write(&file, "a").unwrap();
        let url = format!("file://{}", file.to_string_lossy().replace(' ', "%20"));
        assert_eq!(argument_path(&url, &dir), file);
        assert_eq!(argument_path("a%20b.txt", &dir), dir.join("a%20b.txt"));
    }
}
