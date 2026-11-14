//! Renaming many files at once (Rename… with several selected): rules
//! applied in order to each name, a preview of every new name with any
//! problem shown before anything changes, and renaming that copes with
//! names swapping places (a → b while b → a).
//!
//! The rules, in the order they apply:
//! 1. Find and replace: plain text or a regular expression (with $1 groups
//!    in the replacement), matching case or not, every match or the first.
//! 2. Change case: lower, UPPER, Title Case or Sentence case.
//! 3. Add text before and after.
//! 4. Number them, in the order given (the view's), before or after the
//!    name or in place of it, from a start, by a step, padded with zeros.
//!
//! Each rule works on the name without its extension unless asked to
//! include it; the extension can be kept, changed to lowercase or
//! replaced.

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Rules {
    pub find: String,
    pub replace: String,
    pub regex: bool,
    pub match_case: bool,
    /// Only the first match in each name.
    pub first_only: bool,
    /// none | lower | upper | title | sentence
    pub case: String,
    pub prefix: String,
    pub suffix: String,
    pub number: bool,
    /// before | after | replace
    pub number_at: String,
    pub start: i64,
    pub step: i64,
    /// Digits, padded with zeros (0: as wide as needed).
    pub digits: usize,
    /// Between the number and the name.
    pub separator: String,
    /// The rules work on the extension too.
    pub whole_name: bool,
    /// keep | lower | upper | set
    pub extension: String,
    /// For extension "set" (without the dot; empty: none).
    pub new_extension: String,
}

/// (from, to) for each renamed, or (from, error) for each that failed.
pub type Pairs = Vec<(String, String)>;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Outcome {
    pub from: String,
    pub to: String,
    /// Why it can't be renamed ("" if it can; same name: nothing to do).
    pub problem: String,
}

fn split(name: &str, dir: bool) -> (&str, &str) {
    // Folders and dotfiles without another dot have no extension.
    if dir {
        return (name, "");
    }
    match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i + 1..]),
        _ => (name, ""),
    }
}

fn title(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut start = true;
    for c in s.chars() {
        if start && c.is_alphabetic() {
            out.extend(c.to_uppercase());
            start = false;
        } else {
            out.extend(c.to_lowercase());
            if !c.is_alphanumeric() && c != '\'' {
                start = true;
            } else if c.is_alphanumeric() {
                start = false;
            }
        }
    }
    out
}

fn sentence(s: &str) -> String {
    let lower = s.to_lowercase();
    let mut done = false;
    lower
        .chars()
        .map(|c| {
            if !done && c.is_alphabetic() {
                done = true;
                c.to_uppercase().next().unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

/// The finder for the rules' find text, or an error to show.
fn finder(r: &Rules) -> Result<Option<Regex>, String> {
    if r.find.is_empty() {
        return Ok(None);
    }
    let pattern = if r.regex {
        r.find.clone()
    } else {
        regex::escape(&r.find)
    };
    RegexBuilder::new(&pattern)
        .case_insensitive(!r.match_case)
        .size_limit(1 << 20)
        .build()
        .map(Some)
        .map_err(|e| {
            // The last line says what's wrong; the rest repeats the pattern.
            let msg = e.to_string();
            format!(
                "The pattern isn't valid: {}",
                msg.lines()
                    .last()
                    .unwrap_or("")
                    .trim()
                    .trim_start_matches("error: ")
            )
        })
}

/// The new name for `name` (the `index`th of the batch) under the rules.
fn new_name(r: &Rules, re: Option<&Regex>, name: &str, dir: bool, index: usize) -> String {
    let (stem, ext) = if r.whole_name {
        (name, "")
    } else {
        split(name, dir)
    };
    let mut s = stem.to_string();
    if let Some(re) = re {
        // Plain text: the replacement is literal ($ isn't special).
        s = match (r.regex, r.first_only) {
            (true, true) => re.replace(&s, r.replace.as_str()).into_owned(),
            (true, false) => re.replace_all(&s, r.replace.as_str()).into_owned(),
            (false, true) => re.replace(&s, regex::NoExpand(&r.replace)).into_owned(),
            (false, false) => re.replace_all(&s, regex::NoExpand(&r.replace)).into_owned(),
        };
    }
    s = match r.case.as_str() {
        "lower" => s.to_lowercase(),
        "upper" => s.to_uppercase(),
        "title" => title(&s),
        "sentence" => sentence(&s),
        _ => s,
    };
    s = format!("{}{}{}", r.prefix, s, r.suffix);
    if r.number {
        let n = r.start + r.step * index as i64;
        let num = if n < 0 {
            format!("-{:0width$}", -n, width = r.digits)
        } else {
            format!("{:0width$}", n, width = r.digits)
        };
        s = match r.number_at.as_str() {
            "replace" => num,
            "before" => format!("{num}{}{s}", r.separator),
            _ => format!("{s}{}{num}", r.separator),
        };
    }
    let ext = if r.whole_name || dir {
        String::new()
    } else {
        match r.extension.as_str() {
            "lower" => ext.to_lowercase(),
            "upper" => ext.to_uppercase(),
            "set" => r.new_extension.trim().trim_start_matches('.').to_string(),
            _ => ext.to_string(),
        }
    };
    if ext.is_empty() {
        s
    } else {
        format!("{s}.{ext}")
    }
}

fn name_problem(n: &str) -> Option<&'static str> {
    if n.trim().is_empty() {
        Some("The name would be empty")
    } else if n == "." || n == ".." {
        Some("That name isn't allowed")
    } else if n.contains('/') || n.contains('\0') {
        Some("Names can't contain /")
    } else if n.len() > 255 {
        Some("The name would be too long")
    } else {
        None
    }
}

/// Every file's new name, in the order given. `paths` are in one folder or
/// several; names clash only within a folder. `exists` says whether a name
/// is taken by something that isn't being renamed.
pub fn preview(
    paths: &[String],
    dirs: &[bool],
    r: &Rules,
    exists: &dyn Fn(&Path) -> bool,
) -> Result<Vec<Outcome>, String> {
    let re = finder(r)?;
    let sources: HashSet<PathBuf> = paths.iter().map(PathBuf::from).collect();
    let mut out: Vec<Outcome> = paths
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let path = Path::new(p);
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let to = new_name(
                r,
                re.as_ref(),
                &name,
                dirs.get(i).copied().unwrap_or(false),
                i,
            );
            Outcome {
                from: p.clone(),
                to,
                problem: String::new(),
            }
        })
        .collect();
    // Two ending up with the same name in one folder.
    let mut seen: HashMap<PathBuf, usize> = HashMap::new();
    for i in 0..out.len() {
        let parent = Path::new(&out[i].from)
            .parent()
            .unwrap_or(Path::new("/"))
            .to_path_buf();
        let target = parent.join(&out[i].to);
        if let Some(p) = name_problem(&out[i].to) {
            out[i].problem = p.into();
            continue;
        }
        if let Some(&j) = seen.get(&target) {
            out[i].problem = format!("Same new name as “{}”", file_name(&out[j].from));
            if out[j].problem.is_empty() {
                out[j].problem = format!("Same new name as “{}”", file_name(&out[i].from));
            }
            continue;
        }
        seen.insert(target.clone(), i);
        // Taken by a file that isn't being renamed (one that is moves away).
        if target != Path::new(&out[i].from) && !sources.contains(&target) && exists(&target) {
            out[i].problem = "Something here already has that name".into();
        }
    }
    Ok(out)
}

fn file_name(p: &str) -> String {
    Path::new(p)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Renames by a preview's outcomes (those without a problem that change).
/// When any new name is another's old name, every file steps aside to a
/// temporary name first, so swaps and shifts (1 → 2, 2 → 3) work. Returns (from, to) for each
/// done, and (from, error) for each that failed.
pub fn apply(plan: &[Outcome]) -> (Pairs, Pairs) {
    let todo: Vec<&Outcome> = plan
        .iter()
        .filter(|o| {
            o.problem.is_empty()
                && Path::new(&o.from)
                    .file_name()
                    .map(|n| n.to_string_lossy() != o.to)
                    .unwrap_or(false)
        })
        .collect();
    let target = |o: &Outcome| {
        Path::new(&o.from)
            .parent()
            .unwrap_or(Path::new("/"))
            .join(&o.to)
    };
    let froms: HashSet<PathBuf> = todo.iter().map(|o| PathBuf::from(&o.from)).collect();
    let (mut done, mut failed) = (vec![], vec![]);
    // Step 1: if names move onto each other, all step aside first.
    let crossing = todo.iter().any(|o| froms.contains(&target(o)));
    let mut at: Vec<PathBuf> = vec![];
    for (i, o) in todo.iter().enumerate() {
        let from = PathBuf::from(&o.from);
        if crossing {
            let tmp = from.with_file_name(format!(".wr-rename-{}-{i}", std::process::id()));
            match fs::rename(&from, &tmp) {
                Ok(()) => at.push(tmp),
                Err(e) => {
                    failed.push((o.from.clone(), e.to_string()));
                    at.push(PathBuf::new());
                }
            }
        } else {
            at.push(from);
        }
    }
    // Step 2: everything to its new name.
    for (o, now) in todo.iter().zip(at) {
        if now.as_os_str().is_empty() {
            continue;
        }
        let to = target(o);
        // Never over something that appeared since the preview.
        let clash = fs::symlink_metadata(&to).is_ok() && !same_file(&now, &to);
        let r = if clash {
            Err("something with that name appeared".to_string())
        } else {
            fs::rename(&now, &to).map_err(|e| e.to_string())
        };
        match r {
            Ok(()) => done.push((o.from.clone(), to.to_string_lossy().into_owned())),
            Err(e) => {
                // Back from a temporary name, so nothing is left hidden.
                if now != Path::new(&o.from) {
                    let _ = fs::rename(&now, &o.from);
                }
                failed.push((o.from.clone(), e));
            }
        }
    }
    (done, failed)
}

fn same_file(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (fs::symlink_metadata(a), fs::symlink_metadata(b)) {
        (Ok(x), Ok(y)) => x.ino() == y.ino() && x.dev() == y.dev(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    fn rules() -> Rules {
        Rules {
            case: "none".into(),
            number_at: "after".into(),
            step: 1,
            start: 1,
            extension: "keep".into(),
            ..Default::default()
        }
    }

    fn names(paths: &[&str], r: &Rules) -> Vec<String> {
        let p: Vec<String> = paths.iter().map(|s| s.to_string()).collect();
        preview(&p, &vec![false; p.len()], r, &|_| false)
            .unwrap()
            .into_iter()
            .map(|o| o.to)
            .collect()
    }

    #[test]
    fn find_and_replace_keeps_the_extension() {
        let mut r = rules();
        r.find = "IMG_".into();
        r.replace = "Holiday ".into();
        assert_eq!(
            names(&["/d/IMG_0001.JPG", "/d/img_0002.jpg"], &r),
            ["Holiday 0001.JPG", "Holiday 0002.jpg"]
        );
        r.match_case = true;
        assert_eq!(names(&["/d/img_0002.jpg"], &r), ["img_0002.jpg"]);
        // Plain text: $ and . are just characters.
        let mut r = rules();
        r.find = ".".into();
        r.replace = "$1".into();
        assert_eq!(names(&["/d/a.b.c.txt"], &r), ["a$1b$1c.txt"]);
    }

    #[test]
    fn regex_with_groups() {
        let mut r = rules();
        r.regex = true;
        r.find = r"(\d{4})-(\d{2})-(\d{2})".into();
        r.replace = "$3.$2.$1".into();
        assert_eq!(
            names(&["/d/scan 2026-11-10.pdf"], &r),
            ["scan 10.11.2026.pdf"]
        );
        r.find = "(".into();
        let err = preview(&["/d/a".into()], &[false], &r, &|_| false).unwrap_err();
        assert!(err.starts_with("The pattern isn't valid"), "{err}");
        // First only.
        let mut r = rules();
        r.find = "a".into();
        r.replace = "o".into();
        r.first_only = true;
        assert_eq!(names(&["/d/banana"], &r), ["bonana"]);
    }

    #[test]
    fn case_text_and_numbers() {
        let mut r = rules();
        r.case = "title".into();
        assert_eq!(
            names(&["/d/the QUICK brown-fox.MD"], &r),
            ["The Quick Brown-Fox.MD"]
        );
        r.case = "sentence".into();
        assert_eq!(names(&["/d/the QUICK fox.md"], &r), ["The quick fox.md"]);
        let mut r = rules();
        r.prefix = "2026 ".into();
        r.number = true;
        r.digits = 3;
        r.separator = " - ".into();
        assert_eq!(
            names(&["/d/a.jpg", "/d/b.jpg"], &r),
            ["2026 a - 001.jpg", "2026 b - 002.jpg"]
        );
        r.number_at = "replace".into();
        r.start = 10;
        r.step = 5;
        r.prefix.clear();
        assert_eq!(
            names(&["/d/a.jpg", "/d/b.jpg", "/d/c.jpg"], &r),
            ["010.jpg", "015.jpg", "020.jpg"]
        );
    }

    #[test]
    fn extensions_and_folders() {
        let mut r = rules();
        r.extension = "lower".into();
        r.case = "upper".into();
        assert_eq!(
            names(&["/d/photo.JPG", "/d/.bashrc", "/d/noext"], &r),
            ["PHOTO.jpg", ".BASHRC", "NOEXT"]
        );
        r.extension = "set".into();
        r.new_extension = ".jpeg".into();
        assert_eq!(names(&["/d/a.jpg"], &r), ["A.jpeg"]);
        // Folders have no extension: "v1.2" stays whole.
        let p = vec!["/d/v1.2".to_string()];
        let out = preview(&p, &[true], &r, &|_| false).unwrap();
        assert_eq!(out[0].to, "V1.2");
        // The whole name: the extension changes too.
        let mut r = rules();
        r.whole_name = true;
        r.case = "lower".into();
        assert_eq!(names(&["/d/A.JPG"], &r), ["a.jpg"]);
    }

    #[test]
    fn problems_are_shown_not_done() {
        let mut r = rules();
        r.number = true;
        r.number_at = "replace".into();
        r.step = 0; // every one the same number
        let p: Vec<String> = vec!["/d/a.txt".into(), "/d/b.txt".into()];
        let out = preview(&p, &[false, false], &r, &|_| false).unwrap();
        assert!(
            out[0].problem.contains("b.txt") && out[1].problem.contains("a.txt"),
            "{out:?}"
        );
        // Same name in different folders is fine.
        let p: Vec<String> = vec!["/d/a.txt".into(), "/e/b.txt".into()];
        assert!(preview(&p, &[false, false], &r, &|_| false)
            .unwrap()
            .iter()
            .all(|o| o.problem.is_empty()));
        // Taken by a file that isn't being renamed.
        let mut r = rules();
        r.find = "a".into();
        r.replace = "x".into();
        let out = preview(&["/d/a.txt".into()], &[false], &r, &|p| {
            p == Path::new("/d/x.txt")
        })
        .unwrap();
        assert_eq!(out[0].problem, "Something here already has that name");
        // Emptied, or a slash.
        r.replace = "".into();
        r.whole_name = true;
        r.find = "a.txt".into();
        assert_eq!(
            preview(&["/d/a.txt".into()], &[false], &r, &|_| false).unwrap()[0].problem,
            "The name would be empty"
        );
        r.replace = "x/y".into();
        assert_eq!(
            preview(&["/d/a.txt".into()], &[false], &r, &|_| false).unwrap()[0].problem,
            "Names can't contain /"
        );
    }

    fn tmp(name: &str) -> Scratch {
        let d = Scratch::new(&format!("bulk-{name}"));
        d
    }

    #[test]
    fn renames_including_swaps_and_shifts() {
        let d = tmp("swap");
        for (n, body) in [
            ("1.txt", "one"),
            ("2.txt", "two"),
            ("3.txt", "three"),
            ("keep.txt", "k"),
        ] {
            fs::write(d.join(n), body).unwrap();
        }
        let p = |n: &str| d.join(n).to_string_lossy().into_owned();
        // Shift every number up one: 3 → 4 first would be needed without the temporary step.
        let mut r = rules();
        r.number = true;
        r.number_at = "replace".into();
        r.start = 2;
        let paths = vec![p("1.txt"), p("2.txt"), p("3.txt")];
        let exists = |q: &Path| q.exists();
        let plan = preview(&paths, &[false; 3], &r, &exists).unwrap();
        assert!(plan.iter().all(|o| o.problem.is_empty()), "{plan:?}");
        let (done, failed) = apply(&plan);
        assert!(failed.is_empty(), "{failed:?}");
        assert_eq!(done.len(), 3);
        assert_eq!(fs::read_to_string(d.join("2.txt")).unwrap(), "one");
        assert_eq!(fs::read_to_string(d.join("3.txt")).unwrap(), "two");
        assert_eq!(fs::read_to_string(d.join("4.txt")).unwrap(), "three");
        assert!(!d.join("1.txt").exists());
        // Nothing left behind under a temporary name.
        assert!(fs::read_dir(&d).unwrap().all(|e| !e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".wr-rename")));
    }

    #[test]
    fn never_over_a_file_that_appeared_since() {
        let d = tmp("appeared");
        fs::write(d.join("a.txt"), "a").unwrap();
        let mut r = rules();
        r.find = "a".into();
        r.replace = "b".into();
        let plan = preview(
            &[d.join("a.txt").to_string_lossy().into_owned()],
            &[false],
            &r,
            &|q| q.exists(),
        )
        .unwrap();
        fs::write(d.join("b.txt"), "someone else's").unwrap();
        let (done, failed) = apply(&plan);
        assert!(done.is_empty() && failed.len() == 1);
        assert_eq!(
            fs::read_to_string(d.join("b.txt")).unwrap(),
            "someone else's"
        );
        assert_eq!(fs::read_to_string(d.join("a.txt")).unwrap(), "a");
    }
}
