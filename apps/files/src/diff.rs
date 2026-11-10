//! Line-by-line differences between two text files, for deciding whether
//! to replace one with the other (a clash while copying). Myers' diff
//! (the one git uses), shown as hunks with a few lines of context around
//! each change.

use std::path::Path;

/// Bigger than this (either file), or not text: no diff.
pub const MAX: u64 = 1 << 20;
const CONTEXT: usize = 3;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Line {
    /// " " same, "-" only in the old (what's there), "+" only in the new.
    pub kind: &'static str,
    pub text: String,
    /// 1-based line numbers in the old and new file (0: not in it).
    pub old: usize,
    pub new: usize,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Diff {
    /// Hunks of lines; between hunks, unchanged lines are left out.
    pub hunks: Vec<Vec<Line>>,
    pub added: usize,
    pub removed: usize,
}

/// Text, as far as a diff goes: no NUL bytes in it.
pub fn is_text(b: &[u8]) -> bool {
    !b[..b.len().min(8192)].contains(&0)
}

/// The edit script: for each step, Some((i, j)) a kept line pair, or which
/// side's line goes. Myers' O((N+M)·D) greedy algorithm with a trace.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Op {
    Same(usize, usize),
    Del(usize),
    Ins(usize),
}

fn myers(a: &[&str], b: &[&str]) -> Vec<Op> {
    let (n, m) = (a.len() as isize, b.len() as isize);
    let max = (n + m) as usize;
    let off = max as isize + 1;
    let mut v = vec![0isize; 2 * max + 3];
    let mut trace: Vec<Vec<isize>> = vec![];
    'outer: for d in 0..=max as isize {
        trace.push(v.clone());
        let mut k = -d;
        while k <= d {
            let idx = (k + off) as usize;
            let mut x = if k == -d || (k != d && v[idx - 1] < v[idx + 1]) {
                v[idx + 1]
            } else {
                v[idx - 1] + 1
            };
            let mut y = x - k;
            while x < n && y < m && a[x as usize] == b[y as usize] {
                x += 1;
                y += 1;
            }
            v[idx] = x;
            if x >= n && y >= m {
                trace.push(v.clone());
                break 'outer;
            }
            k += 2;
        }
    }
    // Back through the trace.
    let mut ops = vec![];
    let (mut x, mut y) = (n, m);
    for d in (1..trace.len() - 1).rev() {
        let v = &trace[d];
        let d = d as isize;
        let k = x - y;
        let prev_k = if k == -d || (k != d && v[(k - 1 + off) as usize] < v[(k + 1 + off) as usize])
        {
            k + 1
        } else {
            k - 1
        };
        let prev_x = v[(prev_k + off) as usize];
        let prev_y = prev_x - prev_k;
        while x > prev_x && y > prev_y {
            x -= 1;
            y -= 1;
            ops.push(Op::Same(x as usize, y as usize));
        }
        if x == prev_x {
            y -= 1;
            ops.push(Op::Ins(y as usize));
        } else {
            x -= 1;
            ops.push(Op::Del(x as usize));
        }
    }
    while x > 0 && y > 0 {
        x -= 1;
        y -= 1;
        ops.push(Op::Same(x as usize, y as usize));
    }
    ops.reverse();
    ops
}

/// The differences between `old` and `new` text.
pub fn lines(old: &str, new: &str) -> Diff {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    let ops = myers(&a, &b);
    let all: Vec<Line> = ops
        .iter()
        .map(|op| match *op {
            Op::Same(i, j) => Line {
                kind: " ",
                text: a[i].to_string(),
                old: i + 1,
                new: j + 1,
            },
            Op::Del(i) => Line {
                kind: "-",
                text: a[i].to_string(),
                old: i + 1,
                new: 0,
            },
            Op::Ins(j) => Line {
                kind: "+",
                text: b[j].to_string(),
                old: 0,
                new: j + 1,
            },
        })
        .collect();
    let added = all.iter().filter(|l| l.kind == "+").count();
    let removed = all.iter().filter(|l| l.kind == "-").count();
    // Hunks: each change with CONTEXT lines around it; close ones join.
    let mut keep = vec![false; all.len()];
    for (i, l) in all.iter().enumerate() {
        if l.kind != " " {
            let end = (i + CONTEXT + 1).min(all.len());
            keep[i.saturating_sub(CONTEXT)..end].fill(true);
        }
    }
    let mut hunks: Vec<Vec<Line>> = vec![];
    let mut cur: Vec<Line> = vec![];
    for (l, k) in all.into_iter().zip(keep) {
        if k {
            cur.push(l);
        } else if !cur.is_empty() {
            hunks.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        hunks.push(cur);
    }
    Diff {
        hunks,
        added,
        removed,
    }
}

/// The diff of two files, or why there isn't one.
pub fn files(old: &Path, new: &Path) -> Result<Diff, String> {
    for p in [old, new] {
        let m = std::fs::metadata(p).map_err(|e| e.to_string())?;
        if m.len() > MAX {
            return Err("Too big to compare line by line (over 1 MB)".into());
        }
    }
    let (a, b) = (
        std::fs::read(old).map_err(|e| e.to_string())?,
        std::fs::read(new).map_err(|e| e.to_string())?,
    );
    if !is_text(&a) || !is_text(&b) {
        return Err("Not text, so there are no lines to compare".into());
    }
    Ok(lines(
        &String::from_utf8_lossy(&a),
        &String::from_utf8_lossy(&b),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(d: &Diff) -> Vec<String> {
        d.hunks
            .iter()
            .flatten()
            .map(|l| format!("{}{}", l.kind, l.text))
            .collect()
    }

    #[test]
    fn finds_the_changed_lines() {
        let old = "a\nb\nc\nd\ne\n";
        let new = "a\nb\nC\nd\ne\nf\n";
        let d = lines(old, new);
        assert_eq!((d.added, d.removed), (2, 1));
        assert_eq!(
            flat(&d),
            ["a", "b", "-c", "+C", "d", "e", "+f"].map(|s| if s.len() == 1 {
                format!(" {s}")
            } else {
                s.to_string()
            })
        );
        // Line numbers in each file.
        let c = d.hunks[0].iter().find(|l| l.text == "C").unwrap();
        assert_eq!((c.old, c.new), (0, 3));
    }

    #[test]
    fn far_apart_changes_are_separate_hunks() {
        let old: String = (1..=30).map(|i| format!("{i}\n")).collect();
        let new: String = (1..=30)
            .map(|i| match i {
                2 => "two\n".to_string(),
                28 => "twenty-eight\n".to_string(),
                _ => format!("{i}\n"),
            })
            .collect();
        let d = lines(&old, &new);
        assert_eq!(d.hunks.len(), 2, "{:?}", d.hunks);
        assert_eq!((d.added, d.removed), (2, 2));
        // Context: three lines either side at most.
        assert!(d.hunks[0].len() <= 1 + 1 + 3 + 1);
    }

    #[test]
    fn edge_cases() {
        assert_eq!(lines("", "").hunks.len(), 0);
        let d = lines("", "x\ny\n");
        assert_eq!((d.added, d.removed), (2, 0));
        let d = lines("x\ny\n", "");
        assert_eq!((d.added, d.removed), (0, 2));
        assert_eq!(lines("same\n", "same\n").hunks.len(), 0);
        assert!(!is_text(b"PNG\0\x01"));
    }

    #[test]
    fn matches_a_reference_on_random_edits() {
        // Against a simple LCS: the number of kept lines must be the most possible.
        let mut seed = 7u64;
        let mut rnd = |n: u64| {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 33) % n
        };
        for _ in 0..200 {
            let a: Vec<String> = (0..rnd(12)).map(|_| rnd(4).to_string()).collect();
            let b: Vec<String> = (0..rnd(12)).map(|_| rnd(4).to_string()).collect();
            let (ar, br): (Vec<&str>, Vec<&str>) = (
                a.iter().map(|s| s.as_str()).collect(),
                b.iter().map(|s| s.as_str()).collect(),
            );
            let ops = myers(&ar, &br);
            let kept = ops.iter().filter(|o| matches!(o, Op::Same(..))).count();
            let mut t = vec![vec![0usize; br.len() + 1]; ar.len() + 1];
            for i in (0..ar.len()).rev() {
                for j in (0..br.len()).rev() {
                    t[i][j] = if ar[i] == br[j] {
                        t[i + 1][j + 1] + 1
                    } else {
                        t[i + 1][j].max(t[i][j + 1])
                    };
                }
            }
            assert_eq!(kept, t[0][0], "{a:?} {b:?}");
            // And it rebuilds both sides.
            let rebuilt_a: Vec<&str> = ops
                .iter()
                .filter_map(|o| match o {
                    Op::Same(i, _) | Op::Del(i) => Some(ar[*i]),
                    _ => None,
                })
                .collect();
            let rebuilt_b: Vec<&str> = ops
                .iter()
                .filter_map(|o| match o {
                    Op::Same(_, j) => Some(br[*j]),
                    Op::Ins(j) => Some(br[*j]),
                    _ => None,
                })
                .collect();
            assert_eq!((rebuilt_a, rebuilt_b), (ar.clone(), br.clone()));
        }
    }
}
