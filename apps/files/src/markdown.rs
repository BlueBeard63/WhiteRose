//! Markdown previews: Qt draws Markdown itself (tables, code, lists...),
//! but not Mermaid diagrams or LaTeX maths. Those are drawn by tools when
//! they're installed, nothing bundled: `mmdc` (mermaid-cli) for ```mermaid
//! blocks, `latex` + `dvisvgm` (TeX Live) for $…$ and $$…$$. Each
//! drawing is an SVG kept in ~/.cache/whiterose/files/render, named by a
//! hash of what it draws (and the theme), and put in the Markdown as an
//! image. Without the tools they stay as code, with a hint.

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Mermaid,
    /// Maths on its own line ($$…$$).
    Display,
    /// Maths in a line of text ($…$).
    Inline,
}

/// Something to draw: where it is in the text, and what it says.
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    pub kind: Kind,
    pub start: usize,
    pub end: usize,
    pub source: String,
}

/// The diagrams and maths in `md`, in order. Code (fenced or `inline`) is
/// left alone, except mermaid fences; "$5 and $6" isn't maths (a $ followed
/// by a space or digit after a space doesn't open, nor one before a digit
/// close).
pub fn pieces(md: &str) -> Vec<Piece> {
    let mut out = vec![];
    let b = md.as_bytes();
    let mut i = 0;
    let at_line_start = |i: usize| i == 0 || b[i - 1] == b'\n';
    while i < b.len() {
        // Fenced code.
        if at_line_start(i) && (md[i..].starts_with("```") || md[i..].starts_with("~~~")) {
            let fence = &md[i..i + 3];
            let line_end = md[i..].find('\n').map(|n| i + n).unwrap_or(b.len());
            let info = md[i + 3..line_end].trim().to_lowercase();
            let body_start = (line_end + 1).min(b.len());
            let close = md[body_start..]
                .match_indices(fence)
                .find(|(n, _)| at_line_start(body_start + n))
                .map(|(n, _)| body_start + n);
            let (body_end, end) = match close {
                Some(c) => (c, md[c..].find('\n').map(|n| c + n + 1).unwrap_or(b.len())),
                None => (b.len(), b.len()),
            };
            if info == "mermaid" {
                out.push(Piece {
                    kind: Kind::Mermaid,
                    start: i,
                    end,
                    source: md[body_start..body_end].trim_end().to_string(),
                });
            }
            i = end;
            continue;
        }
        match b[i] {
            b'`' => {
                // Inline code: to the matching run of backticks.
                let n = md[i..].bytes().take_while(|&c| c == b'`').count();
                let tick = &md[i..i + n];
                i = md[i + n..]
                    .find(tick)
                    .map(|e| i + n + e + n)
                    .unwrap_or(i + n);
            }
            b'\\' => i += 2,
            b'$' if md[i..].starts_with("$$") => match md[i + 2..].find("$$") {
                Some(e) if !md[i + 2..i + 2 + e].trim().is_empty() => {
                    out.push(Piece {
                        kind: Kind::Display,
                        start: i,
                        end: i + 2 + e + 2,
                        source: md[i + 2..i + 2 + e].trim().to_string(),
                    });
                    i += e + 4;
                }
                _ => i += 2,
            },
            b'$' => {
                let opens = b
                    .get(i + 1)
                    .is_some_and(|c| !c.is_ascii_whitespace() && !c.is_ascii_digit());
                let line_end = md[i..].find('\n').map(|n| i + n).unwrap_or(b.len());
                let close = opens
                    .then(|| {
                        md[i + 1..line_end]
                            .match_indices('$')
                            .map(|(n, _)| i + 1 + n)
                            .find(|&c| {
                                !b[c - 1].is_ascii_whitespace()
                                    && !b.get(c + 1).is_some_and(|x| x.is_ascii_digit())
                            })
                    })
                    .flatten();
                match close {
                    Some(c) => {
                        out.push(Piece {
                            kind: Kind::Inline,
                            start: i,
                            end: c + 1,
                            source: md[i + 1..c].to_string(),
                        });
                        i = c + 1;
                    }
                    None => i += 1,
                }
            }
            _ => i += md[i..].chars().next().map(char::len_utf8).unwrap_or(1),
        }
    }
    out
}

/// The cached drawing's name for a piece in a theme.
pub fn key(p: &Piece, dark: bool, ink: &str) -> String {
    let mut h = DefaultHasher::new();
    (p.kind.clone() as u8, &p.source, dark, ink, 1u8).hash(&mut h);
    format!("{:016x}.svg", h.finish())
}

pub fn cache_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::places::home().join(".cache"));
    base.join("whiterose/files/render")
}

/// Which tools are installed: (mermaid, maths).
pub fn tools(has: &dyn Fn(&str) -> bool) -> (bool, bool) {
    (has("mmdc"), has("latex") && has("dvisvgm"))
}

/// The Markdown with each piece that has a drawing in `cache` swapped for
/// it; and the pieces still to draw.
pub fn with_drawings(md: &str, cache: &Path, dark: bool, ink: &str) -> (String, Vec<Piece>) {
    let mut out = String::with_capacity(md.len());
    let mut missing = vec![];
    let mut at = 0;
    for p in pieces(md) {
        let file = cache.join(key(&p, dark, ink));
        out.push_str(&md[at..p.start]);
        if file.exists() {
            let url = format!("file://{}", file.display());
            out.push_str(&match p.kind {
                Kind::Mermaid => format!("![diagram]({url})\n"),
                Kind::Display => format!("\n\n![maths]({url})\n\n"),
                Kind::Inline => format!("![maths]({url})"),
            });
        } else {
            out.push_str(&md[p.start..p.end]);
            missing.push(p.clone());
        }
        at = p.end;
    }
    out.push_str(&md[at..]);
    (out, missing)
}

/// Draws one piece into the cache (a tool run; seconds for a diagram).
pub fn draw(p: &Piece, cache: &Path, dark: bool, ink: &str) -> Result<(), String> {
    std::fs::create_dir_all(cache).map_err(|e| e.to_string())?;
    let out = cache.join(key(p, dark, ink));
    let work = tempdir()?;
    let r = match p.kind {
        Kind::Mermaid => {
            let src = work.join("in.mmd");
            std::fs::write(&src, &p.source).map_err(|e| e.to_string())?;
            run(Command::new("mmdc")
                .args(["-i"])
                .arg(&src)
                .arg("-o")
                .arg(work.join("out.svg"))
                .args([
                    "-b",
                    "transparent",
                    "-t",
                    if dark { "dark" } else { "default" },
                ]))
            .and_then(|_| std::fs::rename(work.join("out.svg"), &out).map_err(|e| e.to_string()))
        }
        Kind::Display | Kind::Inline => {
            let maths = if p.kind == Kind::Display {
                format!("\\[{}\\]", p.source)
            } else {
                format!("${}$", p.source)
            };
            let tex = format!(
                "\\documentclass[preview,border=1pt]{{standalone}}\\usepackage{{amsmath,amssymb,xcolor}}\\begin{{document}}\\definecolor{{ink}}{{HTML}}{{{}}}\\color{{ink}}{}\\end{{document}}",
                ink.trim_start_matches('#'),
                maths
            );
            std::fs::write(work.join("m.tex"), tex).map_err(|e| e.to_string())?;
            run(Command::new("latex")
                .args([
                    "-interaction=nonstopmode",
                    "-halt-on-error",
                    "-no-shell-escape",
                    "m.tex",
                ])
                .current_dir(&work))
            .and_then(|_| {
                run(Command::new("dvisvgm")
                    .args(["--no-fonts", "--exact", "-o", "m.svg", "m.dvi"])
                    .current_dir(&work))
            })
            .and_then(|_| std::fs::rename(work.join("m.svg"), &out).map_err(|e| e.to_string()))
        }
    };
    r
}

fn run(c: &mut Command) -> Result<(), String> {
    let o = c
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    if o.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&o.stderr)
            .lines()
            .last()
            .unwrap_or("failed")
            .to_string())
    }
}

/// A drawing's work folder, removed when dropped (also when a step fails
/// part way, so /tmp, often RAM, doesn't collect them).
struct WorkDir(PathBuf);

impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

impl std::ops::Deref for WorkDir {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for WorkDir {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

fn tempdir() -> Result<WorkDir, String> {
    let d = std::env::temp_dir().join(format!(
        "wr-render-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    Ok(WorkDir(d))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    #[test]
    fn finds_diagrams_and_maths_but_not_code_or_money() {
        let md = "# Title\n\nCosts $5 and $6, not maths.\n\nEuler: $e^{i\\pi}+1=0$ and `$not$` here.\n\n$$\n\\int_0^1 x\\,dx\n$$\n\n```mermaid\ngraph TD\n  A-->B\n```\n\n```js\nconst a = \"$x$\";\n```\n\\$escaped$\n";
        let p = pieces(md);
        let kinds: Vec<_> = p
            .iter()
            .map(|x| (x.kind.clone(), x.source.as_str()))
            .collect();
        assert_eq!(
            kinds,
            vec![
                (Kind::Inline, "e^{i\\pi}+1=0"),
                (Kind::Display, "\\int_0^1 x\\,dx"),
                (Kind::Mermaid, "graph TD\n  A-->B"),
            ]
        );
        assert_eq!(&md[p[2].start..p[2].start + 10], "```mermaid");
        assert!(md[p[2].end..].starts_with("\n```js"));
    }

    #[test]
    fn swaps_in_what_is_drawn() {
        let d = Scratch::new("md");
        let md = "a $x^2$ b\n\n```mermaid\ngraph LR\n```\nend";
        let (out, missing) = with_drawings(md, &d, true, "#ffffff");
        assert_eq!(out, md);
        assert_eq!(missing.len(), 2);
        // Drawn (in this theme): swapped for an image.
        std::fs::write(d.join(key(&missing[0], true, "#ffffff")), "<svg/>").unwrap();
        let (out, missing2) = with_drawings(md, &d, true, "#ffffff");
        assert!(
            out.starts_with("a ![maths](file://") && out.contains(".svg) b"),
            "{out}"
        );
        assert_eq!(missing2.len(), 1);
        // Another theme: drawn again.
        assert_eq!(with_drawings(md, &d, false, "#000000").1.len(), 2);
        assert_eq!(tools(&|n| n == "mmdc"), (true, false));
    }
}
