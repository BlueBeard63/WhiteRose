//! Finding files from a terminal, with Files' own search (the same words,
//! filters and index):
//!
//! ```text
//! whiterose-files --find [options] WORDS…
//!   --in DIR        look through DIR's tree as it is now (default: the index of home)
//!   --everywhere    the index of home and the drives
//!   --inside TEXT   the text must be inside the file (contents, a PDF's text, its comment)
//!   --type KIND     folder | document | image | audio | video | archive | code
//!   --changed WHEN  today | week | month | year
//!   --case          letters must match exactly
//!   --regex         WORDS (and --inside) are a regular expression
//!   --hidden        hidden files too
//!   --limit N       at most N (default 50)
//!   --json          one JSON object per line (path, name, dir, size, modified, found)
//! ```
//!
//! WORDS are as in the search box: words in any order, *.pdf / ext:pdf,
//! tag:work. Prints one path per line, best first; exits 1 when nothing
//! was found and 2 on a mistake in what was asked.

use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

use crate::search;

pub const HELP: &str = "whiterose-files --find [--in DIR | --everywhere] [--inside TEXT] [--type KIND] [--changed WHEN] [--case] [--regex] [--hidden] [--limit N] [--json] WORDS…";

#[derive(Debug, Default)]
pub struct Args {
    pub ask: search::Ask,
    pub limit: usize,
    pub json: bool,
}

/// Reads what's after `--find`.
pub fn parse(args: &[String]) -> Result<Args, String> {
    let mut a = Args {
        limit: 50,
        ..Default::default()
    };
    a.ask.scope = "home".into();
    let mut words = vec![];
    let mut it = args.iter();
    let value = |it: &mut std::slice::Iter<String>, what: &str| {
        it.next().cloned().ok_or(format!("{what} needs a value"))
    };
    while let Some(x) = it.next() {
        match x.as_str() {
            "--in" => {
                let d = value(&mut it, "--in")?;
                let p = std::fs::canonicalize(&d).map_err(|_| format!("{d}: no such folder"))?;
                if !p.is_dir() {
                    return Err(format!("{d} isn't a folder"));
                }
                a.ask.scope = "here".into();
                a.ask.here = p.to_string_lossy().into_owned();
            }
            "--everywhere" => a.ask.scope = "everywhere".into(),
            "--inside" => a.ask.content = value(&mut it, "--inside")?,
            "--type" => {
                let k = value(&mut it, "--type")?;
                if ![
                    "folder", "document", "image", "audio", "video", "archive", "code",
                ]
                .contains(&k.as_str())
                {
                    return Err(format!(
                        "--type: {k}? (folder, document, image, audio, video, archive or code)"
                    ));
                }
                a.ask.kind = k;
            }
            "--changed" => {
                let w = value(&mut it, "--changed")?;
                if !["today", "week", "month", "year"].contains(&w.as_str()) {
                    return Err(format!("--changed: {w}? (today, week, month or year)"));
                }
                a.ask.modified = w;
            }
            "--case" => a.ask.case = true,
            "--regex" => a.ask.regex = true,
            "--hidden" => a.ask.hidden = true,
            "--json" => a.json = true,
            "--limit" => {
                a.limit = value(&mut it, "--limit")?
                    .parse()
                    .map_err(|_| "--limit needs a number".to_string())?
            }
            s if s.starts_with("--") => return Err(format!("{s}? ({HELP})")),
            w => words.push(w.to_string()),
        }
    }
    a.ask.text = words.join(" ");
    if a.ask.text.trim().is_empty()
        && a.ask.content.trim().is_empty()
        && a.ask.kind.is_empty()
        && a.ask.modified.is_empty()
    {
        return Err(format!("what to find? ({HELP})"));
    }
    Ok(a)
}

/// Runs the search; the hits, best first.
pub fn find(
    a: &Args,
    index: &Path,
    mime: &dyn Fn(&str) -> String,
    now: i64,
) -> Result<Vec<search::Hit>, String> {
    let q = search::parse_with(&a.ask.text, a.ask.case, a.ask.regex);
    if !q.bad.is_empty() {
        return Err(format!("not a valid pattern: {}", q.bad));
    }
    let f = search::Filters::from(&a.ask, now);
    let stop = AtomicBool::new(false);
    let inside = if a.ask.content.trim().is_empty() {
        None
    } else {
        Some(
            search::Inside::new(&a.ask.content, a.ask.case, a.ask.regex)
                .map_err(|e| format!("not a valid pattern: {e}"))?,
        )
    };
    let home = crate::places::home();
    // A walk: a folder asked for, text inside asked for, or no index yet.
    let ix = if a.ask.scope == "here" || inside.is_some() {
        None
    } else {
        search::Index::open(index)
    };
    if let Some(ix) = ix {
        let want = |top: &Path| a.ask.scope == "everywhere" || top == home;
        let (hits, _) = ix.find(&q, &f, &want, mime, &stop, a.limit);
        return Ok(hits);
    }
    let roots: Vec<PathBuf> = match a.ask.scope.as_str() {
        "here" => vec![PathBuf::from(&a.ask.here)],
        "everywhere" => std::iter::once(home.clone())
            .chain(
                crate::places::drives()
                    .into_iter()
                    .filter(|d| d.mounted && !d.path.is_empty() && d.path != "/")
                    .map(|d| PathBuf::from(d.path)),
            )
            .collect(),
        _ => vec![home],
    };
    let look = search::Look {
        query: &q,
        filters: &f,
        hidden: a.ask.hidden,
        mime,
        inside: inside.as_ref(),
    };
    let mut all = vec![];
    for r in roots {
        search::walk(&r, &look, &stop, a.limit, &mut |h, _, done| {
            if done {
                all.extend(h.iter().cloned());
            }
        });
    }
    search::rank(&mut all, a.limit);
    Ok(all)
}

/// `whiterose-files --find …`: prints and returns the exit code.
pub fn main(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let a = match parse(args) {
        Ok(a) => a,
        Err(e) => {
            let _ = writeln!(err, "whiterose-files: {e}");
            return 2;
        }
    };
    let mime = |n: &str| whiterose_kit::icons::shared().mime.type_of(n).to_string();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let hits = match find(&a, &search::index_file(), &mime, now) {
        Ok(h) => h,
        Err(e) => {
            let _ = writeln!(err, "whiterose-files: {e}");
            return 2;
        }
    };
    for h in &hits {
        let line = if a.json {
            serde_json::json!({"path": h.path, "name": h.name, "dir": h.dir, "size": h.size, "modified": h.modified / 1000, "found": h.found}).to_string()
        } else {
            h.path.clone()
        };
        if writeln!(out, "{line}").is_err() {
            break; // (piped into head)
        }
    }
    if hits.is_empty() {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn reads_options() {
        let a = parse(&s(&[
            "--everywhere",
            "--type",
            "document",
            "--case",
            "--limit",
            "5",
            "tax",
            "*.pdf",
        ]))
        .unwrap();
        assert_eq!(
            (
                a.ask.scope.as_str(),
                a.ask.kind.as_str(),
                a.ask.case,
                a.limit,
                a.ask.text.as_str()
            ),
            ("everywhere", "document", true, 5, "tax *.pdf")
        );
        assert!(parse(&s(&[])).is_err());
        assert!(parse(&s(&["--type", "spreadsheet", "x"])).is_err());
        assert!(parse(&s(&["--in", "/no/such/place", "x"])).is_err());
        assert!(parse(&s(&["--frobnicate"])).is_err());
        assert!(parse(&s(&["--limit"])).is_err());
        // Only inside: fine.
        assert_eq!(
            parse(&s(&["--inside", "invoice"])).unwrap().ask.content,
            "invoice"
        );
    }

    #[test]
    fn finds_in_a_folder() {
        let d = std::env::temp_dir().join(format!("wr-cli-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("Taxes/2025")).unwrap();
        std::fs::write(d.join("Taxes/2025/return.pdf"), "x").unwrap();
        std::fs::write(
            d.join("Taxes/notes.txt"),
            "the tax return is due in January",
        )
        .unwrap();
        std::fs::write(d.join("cat.jpg"), "x").unwrap();
        let mime = |n: &str| {
            if n.ends_with(".pdf") {
                "application/pdf".into()
            } else if n.ends_with(".txt") {
                "text/plain".into()
            } else {
                "image/jpeg".into()
            }
        };
        let dir = d.to_string_lossy().into_owned();
        let run = |args: &[&str]| {
            let mut v = s(&["--in", &dir]);
            v.extend(s(args));
            let a = parse(&v).unwrap();
            find(&a, Path::new("/nonexistent"), &mime, 0)
                .unwrap()
                .into_iter()
                .map(|h| h.name)
                .collect::<Vec<_>>()
        };
        assert_eq!(run(&["return"]), ["return.pdf"]);
        assert_eq!(run(&["*.pdf"]), ["return.pdf"]);
        assert_eq!(run(&["--inside", "January"]), ["notes.txt"]);
        assert_eq!(run(&["--type", "image"]), ["cat.jpg"]);
        // Printed, and the exit code.
        let (mut out, mut err) = (vec![], vec![]);
        assert_eq!(
            main(&s(&["--in", &dir, "--json", "notes"]), &mut out, &mut err),
            0
        );
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["name"], "notes.txt");
        assert_eq!(
            main(
                &s(&["--in", &dir, "nothing-like-this"]),
                &mut vec![],
                &mut vec![]
            ),
            1
        );
        assert_eq!(
            main(&s(&["--in", &dir, "--regex", "("]), &mut vec![], &mut err),
            2
        );
        std::fs::remove_dir_all(&d).unwrap();
    }
}
