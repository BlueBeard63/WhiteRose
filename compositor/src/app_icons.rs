//! Apps' icons for the tab strip: an app id to its desktop entry's Icon=,
//! then the icon theme (yours from GNOME's settings, what it inherits,
//! hicolor) or pixmaps, as the shell finds them. PNG only: an app whose
//! icon is SVG alone shows its title alone. Each icon is read, scaled to
//! the size wanted and premultiplied once, then kept.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};

/// An icon `size` x `size` pixels, premultiplied BGRA (Argb8888 in memory).
#[derive(Debug, PartialEq)]
pub struct Icon {
    pub size: u32,
    pub bgra: Vec<u8>,
}

/// The icon theme sizes looked in, smallest first.
const SIZES: [u32; 11] = [16, 22, 24, 32, 48, 64, 96, 128, 192, 256, 512];

/// `[Desktop Entry]`'s Icon= and StartupWMClass=.
pub fn parse_entry(text: &str) -> (Option<String>, Option<String>) {
    let (mut icon, mut class, mut inside) = (None, None, false);
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            inside = line == "[Desktop Entry]";
            continue;
        }
        if !inside {
            continue;
        }
        if let Some(v) = line.strip_prefix("Icon=") {
            icon = Some(v.trim().to_string()).filter(|v| !v.is_empty());
        } else if let Some(v) = line.strip_prefix("StartupWMClass=") {
            class = Some(v.trim().to_string()).filter(|v| !v.is_empty());
        }
    }
    (icon, class)
}

/// A desktop entry: its file stem and StartupWMClass (lower-cased, for
/// matching) and its Icon=.
type Entry = (String, Option<String>, String);

/// The desktop entries found.
#[derive(Debug, Default)]
pub struct Entries(Vec<Entry>);

impl Entries {
    /// Every `applications/*.desktop` under `data_dirs` (earlier dirs win).
    pub fn scan(data_dirs: &[PathBuf]) -> Entries {
        let mut seen = std::collections::HashSet::new();
        let mut out = vec![];
        for d in data_dirs {
            let Ok(rd) = fs::read_dir(d.join("applications")) else { continue };
            for e in rd.flatten() {
                let p = e.path();
                let Some(stem) = p.file_name().and_then(|n| n.to_str()).and_then(|n| n.strip_suffix(".desktop")) else {
                    continue;
                };
                if !seen.insert(stem.to_string()) {
                    continue;
                }
                let Ok(text) = fs::read_to_string(&p) else { continue };
                if let (Some(icon), class) = parse_entry(&text) {
                    out.push((stem.to_lowercase(), class.map(|c| c.to_lowercase()), icon));
                }
            }
        }
        Entries(out)
    }

    /// The Icon= for `app_id`: the entry named after it, else one whose
    /// window class it is, else one named after its last part
    /// ("org.gnome.Calculator" → "calculator").
    pub fn icon_of(&self, app_id: &str) -> Option<&str> {
        let id = app_id.to_lowercase();
        let last = id.rsplit('.').next().unwrap_or(&id).to_string();
        let find = |f: &dyn Fn(&Entry) -> bool| self.0.iter().find(|e| f(e)).map(|e| e.2.as_str());
        find(&|e| e.0 == id)
            .or_else(|| find(&|e| e.1.as_deref() == Some(id.as_str())))
            .or_else(|| find(&|e| e.0 == last || e.0.rsplit('.').next() == Some(last.as_str())))
    }
}

/// The themes to look in: `theme`, what it inherits (index.theme), hicolor.
pub fn theme_chain(theme: &str, data_dirs: &[PathBuf]) -> Vec<String> {
    let mut chain: Vec<String> = vec![];
    let mut todo = vec![theme.to_string()];
    while let Some(t) = todo.pop() {
        if t.is_empty() || chain.contains(&t) || chain.len() > 8 {
            continue;
        }
        chain.push(t.clone());
        let inherits = data_dirs.iter().find_map(|d| {
            let text = fs::read_to_string(d.join("icons").join(&t).join("index.theme")).ok()?;
            text.lines().find_map(|l| l.trim().strip_prefix("Inherits=").map(str::to_string))
        });
        // (In order: the first inherited is looked in first.)
        for i in inherits.unwrap_or_default().split(',').rev() {
            todo.push(i.trim().to_string());
        }
    }
    if !chain.iter().any(|t| t == "hicolor") {
        chain.push("hicolor".into());
    }
    chain
}

/// The PNG for icon `name` at least `px` big if there's one (else the
/// biggest), in the first theme of `chain` that has it, else pixmaps.
pub fn find_png(name: &str, px: u32, chain: &[String], data_dirs: &[PathBuf]) -> Option<PathBuf> {
    let p = Path::new(name);
    if p.is_absolute() {
        return (p.extension().is_some_and(|e| e == "png") && p.is_file()).then(|| p.to_path_buf());
    }
    for theme in chain {
        let mut found: Vec<(u32, PathBuf)> = vec![];
        for d in data_dirs {
            let base = d.join("icons").join(theme);
            if !base.is_dir() {
                continue;
            }
            for s in SIZES {
                for (dir, real) in
                    [(format!("{s}x{s}"), s), (format!("{s}x{s}@2x"), s * 2), (format!("{s}x{s}@2"), s * 2)]
                {
                    let f = base.join(&dir).join("apps").join(format!("{name}.png"));
                    if f.is_file() {
                        found.push((real, f));
                    }
                }
            }
        }
        found.sort_by_key(|(s, _)| *s);
        if let Some((_, f)) = found.iter().find(|(s, _)| *s >= px).or(found.last()) {
            return Some(f.clone());
        }
    }
    data_dirs.iter().map(|d| d.join("pixmaps").join(format!("{name}.png"))).find(|f| f.is_file())
}

/// A PNG as `px` x `px` premultiplied BGRA (an area average when shrinking).
pub fn load(path: &Path, px: u32) -> Option<Icon> {
    let mut dec = png::Decoder::new(fs::File::open(path).ok()?);
    dec.set_transformations(png::Transformations::normalize_to_color8() | png::Transformations::ALPHA);
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width as usize, info.height as usize);
    let rgba: Vec<[f32; 4]> = match info.color_type {
        png::ColorType::Rgba => {
            buf.chunks(4).take(w * h).map(|c| [c[0] as f32, c[1] as f32, c[2] as f32, c[3] as f32]).collect()
        }
        png::ColorType::GrayscaleAlpha => {
            buf.chunks(2).take(w * h).map(|c| [c[0] as f32, c[0] as f32, c[0] as f32, c[1] as f32]).collect()
        }
        _ => return None,
    };
    let n = px as usize;
    let mut out = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            // The source pixels this one covers (premultiplied, then averaged).
            let (sx0, sx1) = (x * w / n, ((x + 1) * w).div_ceil(n).max(x * w / n + 1).min(w));
            let (sy0, sy1) = (y * h / n, ((y + 1) * h).div_ceil(n).max(y * h / n + 1).min(h));
            let mut acc = [0f32; 4];
            for sy in sy0..sy1 {
                for sx in sx0..sx1 {
                    let [r, g, b, a] = rgba[sy * w + sx];
                    let k = a / 255.0;
                    acc[0] += r * k;
                    acc[1] += g * k;
                    acc[2] += b * k;
                    acc[3] += a;
                }
            }
            let count = ((sx1 - sx0) * (sy1 - sy0)) as f32;
            let i = (y * n + x) * 4;
            for (k, c) in [2, 1, 0, 3].into_iter().enumerate() {
                out[i + k] = (acc[c] / count).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    Some(Icon { size: px, bgra: out })
}

/// $XDG_DATA_HOME and $XDG_DATA_DIRS, as the spec says.
fn data_dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let mut v = vec![std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".local/share"))];
    let dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    v.extend(dirs.split(':').filter(|d| !d.is_empty()).map(PathBuf::from));
    v
}

/// The icon theme GNOME's settings name (Ubuntu and Fedora set it).
fn icon_theme() -> String {
    std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "icon-theme"])
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().trim_matches('\'').to_string())
        .unwrap_or_default()
}

struct Lookup {
    dirs: Vec<PathBuf>,
    entries: Entries,
    chain: Vec<String>,
    icons: HashMap<(String, u32), Option<Arc<Icon>>>,
}

/// `app_id`'s icon at `px` pixels, if it has a PNG one. Looked up once.
pub fn icon(app_id: &str, px: u32) -> Option<Arc<Icon>> {
    static LOOKUP: OnceLock<Mutex<Lookup>> = OnceLock::new();
    let lookup = LOOKUP.get_or_init(|| {
        let dirs = data_dirs();
        let entries = Entries::scan(&dirs);
        let chain = theme_chain(&icon_theme(), &dirs);
        Mutex::new(Lookup { dirs, entries, chain, icons: HashMap::new() })
    });
    let mut l = lookup.lock().unwrap_or_else(|e| e.into_inner());
    let key = (app_id.to_string(), px.max(1));
    if let Some(i) = l.icons.get(&key) {
        return i.clone();
    }
    let found = l
        .entries
        .icon_of(app_id)
        .map(str::to_string)
        .or_else(|| Some(app_id.to_string()))
        .and_then(|name| find_png(&name, key.1, &l.chain, &l.dirs))
        .and_then(|p| load(&p, key.1))
        .map(Arc::new);
    l.icons.insert(key, found.clone());
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    fn scratch(name: &str) -> Scratch {
        let d = Scratch::new(&format!("icons-{name}"));
        d
    }

    fn write_png(path: &Path, size: u32, rgba: [u8; 4]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut e = png::Encoder::new(fs::File::create(path).unwrap(), size, size);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        let data: Vec<u8> = (0..size * size).flat_map(|_| rgba).collect();
        e.write_header().unwrap().write_image_data(&data).unwrap();
    }

    #[test]
    fn reads_desktop_entries() {
        let text = "[Desktop Entry]\nName=Ghostty\nIcon=com.mitchellh.ghostty\nStartupWMClass=ghostty\n[Desktop Action new]\nIcon=other\n";
        assert_eq!(parse_entry(text), (Some("com.mitchellh.ghostty".into()), Some("ghostty".into())));
        assert_eq!(parse_entry("[Desktop Entry]\nName=x\n"), (None, None));
    }

    #[test]
    fn finds_an_apps_icon_name_by_id_class_or_last_part() {
        let d = scratch("entries");
        let apps = d.join("applications");
        fs::create_dir_all(&apps).unwrap();
        fs::write(apps.join("code.desktop"), "[Desktop Entry]\nIcon=vscode\nStartupWMClass=Code\n").unwrap();
        fs::write(apps.join("org.gnome.Calculator.desktop"), "[Desktop Entry]\nIcon=calc\n").unwrap();
        fs::write(apps.join("signal-desktop.desktop"), "[Desktop Entry]\nIcon=signal\nStartupWMClass=Signal\n")
            .unwrap();
        let e = Entries::scan(&[d.to_path_buf()]);
        assert_eq!(e.icon_of("code"), Some("vscode"));
        assert_eq!(e.icon_of("Code"), Some("vscode"), "any case");
        assert_eq!(e.icon_of("signal"), Some("signal"), "its window class");
        assert_eq!(e.icon_of("calculator"), Some("calc"), "its last part");
        assert_eq!(e.icon_of("nothing"), None);
    }

    #[test]
    fn looks_through_the_theme_and_what_it_inherits() {
        let d = scratch("themes");
        fs::create_dir_all(d.join("icons/Mine")).unwrap();
        fs::write(d.join("icons/Mine/index.theme"), "[Icon Theme]\nInherits=Base,Other\n").unwrap();
        fs::create_dir_all(d.join("icons/Base")).unwrap();
        fs::write(d.join("icons/Base/index.theme"), "[Icon Theme]\nInherits=hicolor\n").unwrap();
        let dirs = vec![d.to_path_buf()];
        assert_eq!(theme_chain("Mine", &dirs), vec!["Mine", "Base", "hicolor", "Other"]);
        write_png(&d.join("icons/hicolor/16x16/apps/app.png"), 16, [255, 0, 0, 255]);
        write_png(&d.join("icons/hicolor/48x48/apps/app.png"), 48, [255, 0, 0, 255]);
        write_png(&d.join("icons/Base/24x24@2x/apps/app.png"), 48, [0, 255, 0, 255]);
        write_png(&d.join("pixmaps/old.png"), 32, [0, 0, 255, 255]);
        let chain = theme_chain("Mine", &dirs);
        // The first theme with it, the smallest big enough.
        assert_eq!(find_png("app", 32, &chain, &dirs), Some(d.join("icons/Base/24x24@2x/apps/app.png")));
        assert_eq!(find_png("app", 16, &["hicolor".into()], &dirs), Some(d.join("icons/hicolor/16x16/apps/app.png")));
        assert_eq!(
            find_png("app", 200, &["hicolor".into()], &dirs),
            Some(d.join("icons/hicolor/48x48/apps/app.png")),
            "else the biggest"
        );
        assert_eq!(find_png("old", 16, &chain, &dirs), Some(d.join("pixmaps/old.png")));
        assert_eq!(find_png("none", 16, &chain, &dirs), None);
    }

    #[test]
    fn scales_and_premultiplies() {
        let d = scratch("load");
        let p = d.join("half.png");
        write_png(&p, 64, [200, 100, 50, 128]);
        let i = load(&p, 16).unwrap();
        assert_eq!(i.size, 16);
        assert_eq!(i.bgra.len(), 16 * 16 * 4);
        // B, G, R premultiplied by alpha 128/255, then alpha.
        assert_eq!(&i.bgra[..4], &[25, 50, 100, 128]);
    }
}
