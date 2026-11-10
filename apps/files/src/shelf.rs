//! The shelf: files and folders gathered from anywhere to copy, move or
//! drag somewhere together (paths only; nothing is moved onto it). Kept in
//! $XDG_STATE_HOME/whiterose/files/shelf.json, shared by every window, and
//! anything that's gone since is dropped from it as it's read.

use std::path::{Path, PathBuf};

pub fn file() -> PathBuf {
    crate::session::dir().join("shelf.json")
}

/// What's on it (only what still exists), in the order it was added.
pub fn load(file: &Path) -> Vec<String> {
    let list: Vec<String> = std::fs::read_to_string(file)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    list.into_iter()
        .filter(|p| std::fs::symlink_metadata(p).is_ok())
        .collect()
}

fn save(file: &Path, list: &[String]) -> Result<(), String> {
    if let Some(d) = file.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let tmp = file.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string(list).unwrap_or_default())
        .map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, file).map_err(|e| e.to_string())
}

/// Puts `paths` on it (absolute, existing, not there already; a file
/// inside a folder that's on it is already covered).
pub fn add(file: &Path, paths: &[String]) -> Result<usize, String> {
    let mut list = load(file);
    let mut n = 0;
    for p in paths {
        let path = Path::new(p);
        if !path.is_absolute() || std::fs::symlink_metadata(path).is_err() {
            continue;
        }
        if list.iter().any(|x| path.starts_with(x)) {
            continue;
        }
        // A folder added: what's on it from inside that folder is in it now.
        list.retain(|x| !Path::new(x).starts_with(path));
        list.push(p.clone());
        n += 1;
    }
    save(file, &list)?;
    Ok(n)
}

pub fn remove(file: &Path, paths: &[String]) -> Result<(), String> {
    let mut list = load(file);
    list.retain(|x| !paths.contains(x));
    save(file, &list)
}

pub fn clear(file: &Path) -> Result<(), String> {
    save(file, &[])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gathers_and_forgets() {
        let d = std::env::temp_dir().join(format!("wr-shelf-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("Photos/2026")).unwrap();
        std::fs::write(d.join("Photos/2026/a.jpg"), "a").unwrap();
        std::fs::write(d.join("report.pdf"), "r").unwrap();
        let f = d.join("shelf.json");
        let p = |s: &str| d.join(s).to_string_lossy().into_owned();
        assert_eq!(
            add(
                &f,
                &[
                    p("Photos/2026/a.jpg"),
                    p("report.pdf"),
                    "relative".into(),
                    p("missing")
                ]
            )
            .unwrap(),
            2
        );
        // Again: not twice.
        assert_eq!(add(&f, &[p("report.pdf")]).unwrap(), 0);
        // Its folder: the file inside goes, the folder covers it.
        add(&f, &[p("Photos")]).unwrap();
        assert_eq!(load(&f), vec![p("report.pdf"), p("Photos")]);
        // Something inside a folder that's on it: already covered.
        assert_eq!(add(&f, &[p("Photos/2026")]).unwrap(), 0);
        // Gone since: left out.
        std::fs::remove_file(d.join("report.pdf")).unwrap();
        assert_eq!(load(&f), vec![p("Photos")]);
        remove(&f, &[p("Photos")]).unwrap();
        assert!(load(&f).is_empty());
        add(&f, &[p("Photos")]).unwrap();
        clear(&f).unwrap();
        assert!(load(&f).is_empty());
        std::fs::remove_dir_all(&d).unwrap();
    }
}
