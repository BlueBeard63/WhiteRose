//! Apps that open a kind of file (Properties › Opens with, and Open with in
//! the menu): installed apps' .desktop files, and the defaults and extra
//! associations in mimeapps.list, as the freedesktop.org specs say (so a
//! default set here is the default for xdg-open and every other app too).

use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Entry {
    /// "org.gnome.TextEditor.desktop"
    pub id: String,
    pub name: String,
    pub exec: String,
    pub icon: String,
    pub mimes: Vec<String>,
    pub terminal: bool,
    pub file: PathBuf,
}

/// The [Desktop Entry] of a .desktop file (None if it isn't a shown app).
pub fn parse(id: &str, text: &str, file: &Path) -> Option<Entry> {
    let mut e = Entry {
        id: id.into(),
        file: file.into(),
        ..Entry::default()
    };
    let mut in_main = false;
    let mut kind = String::new();
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            in_main = l == "[Desktop Entry]";
            continue;
        }
        if !in_main || l.starts_with('#') {
            continue;
        }
        let Some((k, v)) = l.split_once('=') else {
            continue;
        };
        let (k, v) = (k.trim(), v.trim());
        match k {
            "Type" => kind = v.into(),
            "Name" => e.name = v.into(),
            "Exec" => e.exec = v.into(),
            "Icon" => e.icon = v.into(),
            "Terminal" => e.terminal = v == "true",
            "MimeType" => {
                e.mimes = v
                    .split(';')
                    .filter(|m| !m.is_empty())
                    .map(str::to_string)
                    .collect()
            }
            "NoDisplay" | "Hidden" if v == "true" => return None,
            "TryExec" if !v.is_empty() && !crate::launch::installed(v) => return None,
            _ => {}
        }
    }
    (kind == "Application" && !e.exec.is_empty() && !e.name.is_empty()).then_some(e)
}

fn env_dirs(var: &str, default: &str) -> Vec<PathBuf> {
    std::env::var(var)
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| default.into())
        .split(':')
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect()
}

fn data_home() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::places::home().join(".local/share"))
}

fn config_home() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::places::home().join(".config"))
}

/// Where applications live, most important first.
pub fn app_dirs() -> Vec<PathBuf> {
    std::iter::once(data_home())
        .chain(env_dirs("XDG_DATA_DIRS", "/usr/local/share:/usr/share"))
        .map(|d| d.join("applications"))
        .collect()
}

/// Every app, by id (the first found wins: yours over the system's).
pub fn all(dirs: &[PathBuf]) -> BTreeMap<String, Entry> {
    fn walk(
        root: &Path,
        dir: &Path,
        out: &mut BTreeMap<String, Entry>,
        seen: &mut std::collections::HashSet<String>,
    ) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(root, &p, out, seen);
                continue;
            }
            if p.extension().is_none_or(|x| x != "desktop") {
                continue;
            }
            // Subfolders join with "-" (the spec's desktop-file id).
            let id = p
                .strip_prefix(root)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('/', "-");
            if !seen.insert(id.clone()) {
                continue;
            }
            if let Some(entry) = fs::read_to_string(&p).ok().and_then(|t| parse(&id, &t, &p)) {
                out.insert(id, entry);
            }
        }
    }
    let mut out = BTreeMap::new();
    let mut seen = std::collections::HashSet::new();
    for d in dirs {
        walk(d, d, &mut out, &mut seen);
    }
    out
}

/// What mimeapps.list files say: defaults, added and removed associations.
#[derive(Debug, Default, PartialEq)]
pub struct Lists {
    pub defaults: HashMap<String, Vec<String>>,
    pub added: HashMap<String, Vec<String>>,
    pub removed: HashMap<String, Vec<String>>,
}

pub fn list_files() -> Vec<PathBuf> {
    files_for(
        &desktops(),
        config_home(),
        env_dirs("XDG_CONFIG_DIRS", "/etc/xdg"),
        app_dirs(),
    )
}

/// The session's desktops, lowercase ("whiterose"; "ubuntu", "gnome").
pub fn desktops() -> Vec<String> {
    std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .filter(|d| !d.is_empty())
        .map(|d| d.to_lowercase())
        .collect()
}

/// In each folder, the desktops' own lists first, then the plain one
/// (the spec's order).
pub fn files_for(
    desktops: &[String],
    config: PathBuf,
    config_dirs: Vec<PathBuf>,
    app_dirs: Vec<PathBuf>,
) -> Vec<PathBuf> {
    let mut v = vec![];
    for d in std::iter::once(config).chain(config_dirs).chain(app_dirs) {
        for name in desktops {
            v.push(d.join(format!("{name}-mimeapps.list")));
        }
        v.push(d.join("mimeapps.list"));
    }
    v
}

/// Where a default you pick is saved: in a WhiteRose session WhiteRose's
/// own list (Settings › Apps › Default apps keeps it there too; GNOME's
/// defaults stay as they are), else your mimeapps.list.
pub fn user_list() -> PathBuf {
    if desktops().iter().any(|d| d == "whiterose") {
        config_home().join("whiterose-mimeapps.list")
    } else {
        config_home().join("mimeapps.list")
    }
}

/// Read in order: earlier files win for defaults; associations add up.
pub fn read_lists(files: &[PathBuf]) -> Lists {
    let mut l = Lists::default();
    for f in files {
        let Ok(text) = fs::read_to_string(f) else {
            continue;
        };
        let mut section = "";
        for line in text.lines() {
            let t = line.trim();
            if t.starts_with('[') {
                section = match t {
                    "[Default Applications]" => "d",
                    "[Added Associations]" => "a",
                    "[Removed Associations]" => "r",
                    _ => "",
                };
                continue;
            }
            let Some((mime, ids)) = t.split_once('=') else {
                continue;
            };
            let ids: Vec<String> = ids
                .split(';')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();
            let map = match section {
                "d" => &mut l.defaults,
                "a" => &mut l.added,
                "r" => &mut l.removed,
                _ => continue,
            };
            if section == "d" {
                map.entry(mime.trim().to_string()).or_insert(ids);
            } else {
                map.entry(mime.trim().to_string()).or_default().extend(ids);
            }
        }
    }
    l
}

#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct Choice {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub default: bool,
}

/// The apps for `mime`, the default first (then by name). Text files also
/// get the apps for plain text.
pub fn for_mime(mime: &str, apps: &BTreeMap<String, Entry>, lists: &Lists) -> Vec<Choice> {
    let mut wanted = vec![mime.to_string()];
    if mime.starts_with("text/") && mime != "text/plain" {
        wanted.push("text/plain".into());
    }
    let removed: Vec<&String> = wanted
        .iter()
        .flat_map(|m| lists.removed.get(m).into_iter().flatten())
        .collect();
    let mut ids: Vec<String> = vec![];
    for m in &wanted {
        for id in lists.added.get(m).into_iter().flatten() {
            if !ids.contains(id) {
                ids.push(id.clone());
            }
        }
        for (id, e) in apps {
            if e.mimes.iter().any(|x| x == m) && !ids.contains(id) {
                ids.push(id.clone());
            }
        }
    }
    let default = wanted
        .iter()
        .find_map(|m| {
            lists
                .defaults
                .get(m)
                .and_then(|d| d.iter().find(|id| apps.contains_key(*id)))
        })
        .cloned()
        .or_else(|| {
            ids.iter()
                .find(|id| apps.contains_key(*id) && !removed.contains(id))
                .cloned()
        });
    let mut out: Vec<Choice> = ids
        .into_iter()
        .filter(|id| !removed.contains(&id) || Some(id) == default.as_ref())
        .filter_map(|id| {
            let e = apps.get(&id)?;
            Some(Choice {
                default: Some(&id) == default.as_ref(),
                id,
                name: e.name.clone(),
                icon: e.icon.clone(),
            })
        })
        .collect();
    out.sort_by(|a, b| {
        b.default
            .cmp(&a.default)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out
}

/// Makes `id` the default for `mime` in your mimeapps.list (everything else
/// in it kept).
pub fn set_default(file: &Path, mime: &str, id: &str) -> Result<(), String> {
    let text = fs::read_to_string(file).unwrap_or_default();
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let line = format!("{mime}={id};");
    let start = lines
        .iter()
        .position(|l| l.trim() == "[Default Applications]");
    match start {
        Some(s) => {
            let end = lines[s + 1..]
                .iter()
                .position(|l| l.trim().starts_with('['))
                .map(|e| s + 1 + e)
                .unwrap_or(lines.len());
            match lines[s + 1..end]
                .iter()
                .position(|l| l.split_once('=').is_some_and(|(m, _)| m.trim() == mime))
            {
                Some(i) => lines[s + 1 + i] = line,
                None => lines.insert(s + 1, line),
            }
        }
        None => {
            if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push("[Default Applications]".into());
            lines.push(line);
        }
    }
    if let Some(d) = file.parent() {
        fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let tmp = file.with_extension("list.tmp");
    fs::write(&tmp, lines.join("\n") + "\n").map_err(|e| e.to_string())?;
    fs::rename(&tmp, file).map_err(|e| e.to_string())
}

/// Splits Exec into words: double quotes group, and inside them \" \` \$ \\
/// stand for themselves.
fn words(exec: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut quoted = false;
    let mut any = false;
    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            '\\' if quoted => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            c if c.is_whitespace() && !quoted => {
                if any || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            c => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn uri(p: &str) -> String {
    format!("file://{}", crate::places::percent_encode(p))
}

/// The command lines that open `files` with `e`: one, or one per file when
/// it takes a single file (%f / %u) and several are given.
pub fn command_lines(e: &Entry, files: &[String]) -> Vec<Vec<String>> {
    let w = words(&e.exec);
    let multi = w.iter().any(|x| x.contains("%F") || x.contains("%U"));
    let single = w.iter().any(|x| x.contains("%f") || x.contains("%u"));
    let groups: Vec<Vec<String>> = if !multi && single {
        files.iter().map(|f| vec![f.clone()]).collect()
    } else {
        vec![files.to_vec()]
    };
    groups
        .into_iter()
        .map(|group| {
            let mut out = vec![];
            for word in &w {
                match word.as_str() {
                    "%F" => out.extend(group.iter().cloned()),
                    "%U" => out.extend(group.iter().map(|f| uri(f))),
                    "%f" => out.extend(group.first().cloned()),
                    "%u" => out.extend(group.first().map(|f| uri(f))),
                    "%i" if !e.icon.is_empty() => {
                        out.extend(["--icon".to_string(), e.icon.clone()])
                    }
                    _ => {
                        // Field codes inside a word; ones it doesn't use go.
                        let mut s = String::new();
                        let mut it = word.chars();
                        while let Some(c) = it.next() {
                            if c != '%' {
                                s.push(c);
                                continue;
                            }
                            match it.next() {
                                Some('%') => s.push('%'),
                                Some('c') => s.push_str(&e.name),
                                Some('k') => s.push_str(&e.file.to_string_lossy()),
                                Some('f') => {
                                    s.push_str(group.first().map(String::as_str).unwrap_or(""))
                                }
                                Some('u') => {
                                    s.push_str(&group.first().map(|f| uri(f)).unwrap_or_default())
                                }
                                _ => {}
                            }
                        }
                        if !s.is_empty() {
                            out.push(s);
                        }
                    }
                }
            }
            out
        })
        .filter(|c| !c.is_empty())
        .collect()
}

type Held = Option<(
    std::time::Instant,
    std::sync::Arc<(BTreeMap<String, Entry>, Lists)>,
)>;
static CACHE: std::sync::Mutex<Held> = std::sync::Mutex::new(None);

/// Installed apps and the lists, read at most every few seconds (a menu
/// opening, then Properties, read them once).
pub fn cached() -> std::sync::Arc<(BTreeMap<String, Entry>, Lists)> {
    let mut c = CACHE.lock().unwrap();
    if let Some((at, v)) = c.as_ref() {
        if at.elapsed() < std::time::Duration::from_secs(5) {
            return v.clone();
        }
    }
    let v = std::sync::Arc::new((all(&app_dirs()), read_lists(&list_files())));
    *c = Some((std::time::Instant::now(), v.clone()));
    v
}

/// Forgets what was read (a default was just changed).
pub fn forget() {
    *CACHE.lock().unwrap() = None;
}

/// Opens `files` with the app `id` (terminal apps in a terminal).
pub fn open_with(id: &str, files: &[String], terminal: Option<&str>) -> Result<(), String> {
    let held = cached();
    let e = held.0.get(id).ok_or("That app isn't installed any more")?;
    let dir = files
        .first()
        .and_then(|f| Path::new(f).parent())
        .map(|d| d.to_string_lossy().into_owned())
        .unwrap_or_default();
    for line in command_lines(e, files) {
        let line = if e.terminal {
            let t = terminal.ok_or("It runs in a terminal, and none is installed")?;
            let mut v = vec![t.to_string(), "-e".into()];
            v.extend(line);
            v
        } else {
            line
        };
        crate::launch::spawn(&line[0], &line[1..], &dir)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    fn entry(id: &str, name: &str, exec: &str, mimes: &[&str]) -> Entry {
        Entry {
            id: id.into(),
            name: name.into(),
            exec: exec.into(),
            mimes: mimes.iter().map(|s| s.to_string()).collect(),
            file: PathBuf::from(format!("/usr/share/applications/{id}")),
            ..Entry::default()
        }
    }

    #[test]
    fn parses_entries() {
        let t = "[Desktop Entry]\nType=Application\nName=Text Editor\nName[de]=Texteditor\nExec=gnome-text-editor %U\nIcon=org.gnome.TextEditor\nMimeType=text/plain;text/markdown;\n\n[Desktop Action new-window]\nExec=other\n";
        let e = parse("te.desktop", t, Path::new("/x/te.desktop")).unwrap();
        assert_eq!(e.name, "Text Editor");
        assert_eq!(e.exec, "gnome-text-editor %U");
        assert_eq!(e.mimes, vec!["text/plain", "text/markdown"]);
        assert!(parse(
            "h.desktop",
            "[Desktop Entry]\nType=Application\nName=H\nExec=h\nNoDisplay=true\n",
            Path::new("/h")
        )
        .is_none());
        assert!(parse(
            "l.desktop",
            "[Desktop Entry]\nType=Link\nName=L\nURL=x\n",
            Path::new("/l")
        )
        .is_none());
    }

    #[test]
    fn choices_defaults_and_associations() {
        let mut apps = BTreeMap::new();
        for e in [
            entry("a.desktop", "Alpha", "a %f", &["text/plain"]),
            entry("b.desktop", "Beta", "b %F", &["text/markdown"]),
            entry("c.desktop", "Gamma", "c %U", &["image/png"]),
            entry("d.desktop", "Delta", "d", &["text/plain"]),
        ] {
            apps.insert(e.id.clone(), e);
        }
        let d = Scratch::new("apps");
        let mine = d.join("mimeapps.list");
        fs::write(&mine, "[Added Associations]\ntext/markdown=c.desktop;\n[Removed Associations]\ntext/plain=d.desktop;\n").unwrap();
        let sys = d.join("sys.list");
        fs::write(&sys, "[Default Applications]\ntext/markdown=b.desktop;\n").unwrap();
        let l = read_lists(&[mine.clone(), sys.clone()]);
        let names = |v: Vec<Choice>| {
            v.into_iter()
                .map(|c| (c.name, c.default))
                .collect::<Vec<_>>()
        };
        // Markdown: its default first, then added and plain-text apps; Delta removed.
        assert_eq!(
            names(for_mime("text/markdown", &apps, &l)),
            vec![
                ("Beta".into(), true),
                ("Alpha".into(), false),
                ("Gamma".into(), false)
            ]
        );
        // Your default (in your file) beats the system's, and the file keeps the rest.
        set_default(&mine, "text/markdown", "a.desktop").unwrap();
        set_default(&mine, "image/png", "c.desktop").unwrap();
        let text = fs::read_to_string(&mine).unwrap();
        assert!(
            text.contains("[Removed Associations]\ntext/plain=d.desktop;"),
            "{text}"
        );
        let l = read_lists(&[mine.clone(), sys]);
        assert_eq!(for_mime("text/markdown", &apps, &l)[0].name, "Alpha");
        set_default(&mine, "text/markdown", "b.desktop").unwrap();
        assert_eq!(
            fs::read_to_string(&mine)
                .unwrap()
                .matches("text/markdown=")
                .count(),
            2,
            "replaced, not added (one default, one added association)"
        );
    }

    #[test]
    fn exec_lines() {
        let e = entry(
            "x.desktop",
            "X Viewer",
            r#"xv --title "%c here" --x\ y %f"#,
            &[],
        );
        let files = vec!["/a b/1.png".to_string(), "/2.png".to_string()];
        assert_eq!(
            command_lines(&e, &files),
            vec![
                vec!["xv", "--title", "X Viewer here", "--x\\", "y", "/a b/1.png"],
                vec!["xv", "--title", "X Viewer here", "--x\\", "y", "/2.png"],
            ]
        );
        let e = entry("y.desktop", "Y", "y --new %U", &[]);
        assert_eq!(
            command_lines(&e, &files),
            vec![vec!["y", "--new", "file:///a%20b/1.png", "file:///2.png"]]
        );
        let e = entry("z.desktop", "Z", r#"sh -c "echo \"\$1\"" %%"#, &[]);
        assert_eq!(
            command_lines(&e, &[]),
            vec![vec!["sh", "-c", "echo \"$1\"", "%"]]
        );
    }

    #[test]
    fn desktop_lists_first() {
        let v = files_for(
            &["whiterose".to_string()],
            PathBuf::from("/h/.config"),
            vec![PathBuf::from("/etc/xdg")],
            vec![PathBuf::from("/usr/share/applications")],
        );
        let names: Vec<String> = v.iter().map(|p| p.to_string_lossy().into_owned()).collect();
        assert_eq!(
            names,
            [
                "/h/.config/whiterose-mimeapps.list",
                "/h/.config/mimeapps.list",
                "/etc/xdg/whiterose-mimeapps.list",
                "/etc/xdg/mimeapps.list",
                "/usr/share/applications/whiterose-mimeapps.list",
                "/usr/share/applications/mimeapps.list"
            ]
        );
    }
}
