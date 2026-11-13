//! `whiterose-files --choose`: Files as a picker, for Settings and (through
//! whiterosed's FileChooser portal) every app. A Files window with a strip
//! above the files says what's wanted; the chosen paths are printed (one a
//! line, or a JSON array with --json) and it exits 0. Cancelled (or the
//! window closed): nothing printed, exit 1.
//!
//!   whiterose-files --choose [--folder | --save NAME] [--multiple]
//!       [--in DIR] [--title TEXT] [--filter "NAME:PATTERN …"]… [--json]
//!
//! A filter's patterns are globs on the name (`*.png`) or MIME types
//! (`image/*`); the first filter is the one shown first. Folders are always
//! shown, to go into; with --folder, only folders are.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct Filter {
    pub name: String,
    pub patterns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Choose {
    /// open | folder | save
    pub mode: String,
    pub multiple: bool,
    /// Where it starts ("" for home).
    pub folder: String,
    pub title: String,
    /// The name to save as, to start with.
    pub name: String,
    pub filters: Vec<Filter>,
    pub json: bool,
}

/// The options after `--choose`, or what's wrong with them.
pub fn parse(args: &[String]) -> Result<Choose, String> {
    let mut c = Choose {
        mode: "open".into(),
        multiple: false,
        folder: String::new(),
        title: String::new(),
        name: String::new(),
        filters: vec![],
        json: false,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = |what: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{a} needs {what}"))
        };
        match a.as_str() {
            "--folder" => c.mode = "folder".into(),
            "--save" => {
                c.mode = "save".into();
                c.name = value("a name")?;
            }
            "--multiple" => c.multiple = true,
            "--json" => c.json = true,
            "--in" => c.folder = value("a folder")?,
            "--title" => c.title = value("a title")?,
            "--filter" => {
                let f = value("NAME:PATTERNS")?;
                let (name, pats) = f
                    .split_once(':')
                    .ok_or_else(|| format!("--filter {f:?}: NAME:PATTERNS"))?;
                let patterns: Vec<String> = pats.split_whitespace().map(str::to_string).collect();
                if patterns.is_empty() {
                    return Err(format!("--filter {f:?} has no patterns"));
                }
                c.filters.push(Filter {
                    name: name.trim().to_string(),
                    patterns,
                });
            }
            other => return Err(format!("unknown option {other}")),
        }
    }
    if c.mode != "open" {
        c.multiple = false;
    }
    if c.title.is_empty() {
        c.title = match c.mode.as_str() {
            "folder" => "Choose a folder",
            "save" => "Save as",
            _ if c.multiple => "Choose files",
            _ => "Choose a file",
        }
        .into();
    }
    if let Some(rest) = c.folder.strip_prefix("file://") {
        c.folder = rest.to_string();
    }
    Ok(c)
}

/// What's printed for `paths`.
pub fn output(paths: &[String], json: bool) -> String {
    if json {
        serde_json::to_string(paths).unwrap_or_else(|_| "[]".into()) + "\n"
    } else {
        paths.iter().map(|p| format!("{p}\n")).collect()
    }
}

/// True if `name` (with `mime`) is one of `filter`'s.
pub fn matches(filter: &Filter, name: &str, mime: &str) -> bool {
    filter.patterns.iter().any(|p| {
        if p.contains('/') {
            match p.strip_suffix("/*") {
                Some(top) => mime.split('/').next() == Some(top),
                None => p == mime,
            }
        } else {
            glob(&p.to_lowercase(), &name.to_lowercase())
        }
    })
}

/// One piece of a glob.
enum Tok {
    Star,
    Any,
    Char(char),
    /// `[abc]`, `[a-z]`; `[!…]` or `[^…]`: none of them.
    Class(Vec<(char, char)>, bool),
}

fn tokens(p: &str) -> Vec<Tok> {
    let c: Vec<char> = p.chars().collect();
    let mut out = vec![];
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            '*' => out.push(Tok::Star),
            '?' => out.push(Tok::Any),
            '[' => {
                let mut j = i + 1;
                let not = j < c.len() && (c[j] == '!' || c[j] == '^');
                if not {
                    j += 1;
                }
                let start = j;
                // (A `]` first is one of the set.)
                while j < c.len() && (c[j] != ']' || j == start) {
                    j += 1;
                }
                if j >= c.len() {
                    out.push(Tok::Char('['));
                } else {
                    let set = &c[start..j];
                    let mut ranges = vec![];
                    let mut k = 0;
                    while k < set.len() {
                        if k + 2 < set.len() && set[k + 1] == '-' {
                            ranges.push((set[k], set[k + 2]));
                            k += 3;
                        } else {
                            ranges.push((set[k], set[k]));
                            k += 1;
                        }
                    }
                    out.push(Tok::Class(ranges, not));
                    i = j;
                }
            }
            ch => out.push(Tok::Char(ch)),
        }
        i += 1;
    }
    out
}

/// `*`, `?` and `[…]` globs, as GTK apps give them (`*.[pP][nN][gG]`).
fn glob(p: &str, s: &str) -> bool {
    let p = tokens(p);
    let s: Vec<char> = s.chars().collect();
    let one = |t: &Tok, ch: char| match t {
        Tok::Any => true,
        Tok::Char(c) => *c == ch,
        Tok::Class(r, not) => r.iter().any(|(a, b)| (*a..=*b).contains(&ch)) != *not,
        Tok::Star => false,
    };
    let (mut pi, mut si, mut star, mut mark) = (0, 0, None, 0);
    while si < s.len() {
        if pi < p.len() && one(&p[pi], s[si]) {
            pi += 1;
            si += 1;
        } else if pi < p.len() && matches!(p[pi], Tok::Star) {
            star = Some(pi);
            mark = si;
            pi += 1;
        } else if let Some(st) = star {
            pi = st + 1;
            mark += 1;
            si = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|t| matches!(t, Tok::Star))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_what_is_wanted() {
        let c = parse(&args(&[
            "--title",
            "Choose a picture",
            "--in",
            "file:///home/me/Pictures",
            "--filter",
            "Images:*.png *.JPG image/*",
        ]))
        .unwrap();
        assert_eq!(c.mode, "open");
        assert_eq!(c.title, "Choose a picture");
        assert_eq!(c.folder, "/home/me/Pictures");
        assert_eq!(
            c.filters,
            vec![Filter {
                name: "Images".into(),
                patterns: args(&["*.png", "*.JPG", "image/*"])
            }]
        );

        let s = parse(&args(&["--save", "notes.txt", "--multiple"])).unwrap();
        assert_eq!(
            (
                s.mode.as_str(),
                s.name.as_str(),
                s.multiple,
                s.title.as_str()
            ),
            ("save", "notes.txt", false, "Save as")
        );
        assert_eq!(
            parse(&args(&["--folder"])).unwrap().title,
            "Choose a folder"
        );
        assert_eq!(parse(&args(&["--multiple"])).unwrap().title, "Choose files");

        assert!(parse(&args(&["--save"])).is_err(), "no name");
        assert!(
            parse(&args(&["--filter", "Images"])).is_err(),
            "no patterns"
        );
        assert!(parse(&args(&["--frobnicate"])).is_err());
    }

    #[test]
    fn matches_globs_and_types() {
        let f = Filter {
            name: "Pictures".into(),
            patterns: args(&["*.png", "*.jp?g", "image/webp"]),
        };
        assert!(matches(&f, "Me.PNG", "image/png"));
        assert!(matches(&f, "a.jpeg", ""));
        assert!(matches(&f, "x", "image/webp"));
        assert!(!matches(&f, "a.png.txt", "text/plain"));
        let any = Filter {
            name: "Images".into(),
            patterns: args(&["image/*"]),
        };
        assert!(matches(&any, "photo", "image/heic"));
        assert!(!matches(&any, "photo", "imagery/x"));
    }

    #[test]
    fn prints_lines_or_json() {
        let p = args(&["/a b/c.png", "/d"]);
        assert_eq!(output(&p, false), "/a b/c.png\n/d\n");
        assert_eq!(output(&p, true), "[\"/a b/c.png\",\"/d\"]\n");
    }
}
