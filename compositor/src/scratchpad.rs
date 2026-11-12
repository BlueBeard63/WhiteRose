//! Scratchpads (`windows.scratchpads`): apps shown and hidden with a key,
//! sliding down from the island.
//!
//! A scratchpad is the window its key started: the key runs the command,
//! and the first window that process (or a child of it) opens is claimed.
//! It isn't on any workspace; it shows on the monitor you're working on.
//! Pure: parsing, what the key does, where the window goes and the pid
//! check, tested without a compositor.

use std::time::{Duration, Instant};

use serde_json::Value;

use crate::{layout::Rect, rules::Len};

/// How long a start may take to open its window before it's given up.
pub const START_TIMEOUT: Duration = Duration::from_secs(10);

/// Where a scratchpad sits in the monitor's usable area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Place {
    /// Centred, against the top: under the island.
    #[default]
    Top,
    Centre,
}

/// One scratchpad from the setting.
#[derive(Debug, Clone, PartialEq)]
pub struct Pad {
    pub id: String,
    pub name: String,
    pub command: String,
    pub keys: Vec<String>,
    pub size: (Len, Len),
    pub place: Place,
    /// Hide it when it loses focus (else it stays until its key).
    pub hide_on_blur: bool,
}

const DEFAULT_SIZE: (Len, Len) = (Len::Fraction(0.6), Len::Fraction(0.5));

impl Pad {
    fn parse(v: &Value) -> Option<Pad> {
        let o = v.as_object()?;
        let text = |k: &str| o.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());
        let size = o
            .get("size")
            .and_then(Value::as_array)
            .filter(|a| a.len() == 2)
            .and_then(|a| Some((Len::parse(&a[0], 1)?, Len::parse(&a[1], 1)?)));
        Some(Pad {
            id: text("id")?.to_owned(),
            name: text("name").unwrap_or_default().to_owned(),
            command: text("command")?.to_owned(),
            keys: o
                .get("keys")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect())
                .unwrap_or_default(),
            size: size.unwrap_or(DEFAULT_SIZE),
            place: match o.get("place").and_then(Value::as_str) {
                Some("centre" | "center") => Place::Centre,
                _ => Place::Top,
            },
            hide_on_blur: o.get("hideOnBlur").and_then(Value::as_bool).unwrap_or(false),
        })
    }

    /// Its rectangle in a monitor's usable `area`: its size cut to fit,
    /// centred across, and against the top or in the middle.
    pub fn rect(&self, area: Rect) -> Rect {
        let w = self.size.0.of(area.w).clamp(1, area.w.max(1));
        let h = self.size.1.of(area.h).clamp(1, area.h.max(1));
        let (x, y) = self.origin(area, (w, h));
        Rect::new(x, y, w, h)
    }

    /// Where its top left goes for a window of `size` (the size it drew,
    /// which an app may choose for itself): centred across, against the
    /// top or in the middle, and kept inside the area.
    pub fn origin(&self, area: Rect, size: (i32, i32)) -> (i32, i32) {
        let (w, h) = size;
        let x = area.x + (area.w - w) / 2;
        let y = match self.place {
            Place::Top => area.y,
            Place::Centre => area.y + (area.h - h) / 2,
        };
        (x.max(area.x), y.max(area.y))
    }
}

/// Reads `windows.scratchpads`. One without an id or a command, or with an
/// id already used, is skipped (and logged).
pub fn parse(v: &Value) -> Vec<Pad> {
    let mut pads: Vec<Pad> = Vec::new();
    for (i, p) in v.as_array().into_iter().flatten().enumerate() {
        match Pad::parse(p) {
            Some(pad) if pads.iter().any(|q| q.id == pad.id) => {
                tracing::warn!(scratchpad = i + 1, id = pad.id, "skipping a scratchpad whose id is used twice");
            }
            Some(pad) => pads.push(pad),
            None => tracing::warn!(scratchpad = i + 1, "skipping a scratchpad with no id or command"),
        }
    }
    pads
}

/// Where a scratchpad is. `W` is its window, `M` a monitor.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum State<W, M> {
    /// Not running: the key starts it.
    #[default]
    Off,
    /// Started; waiting for that process's first window.
    Starting {
        pid: u32,
        since: Instant,
    },
    Hidden(W),
    Shown {
        window: W,
        on: M,
    },
}

/// What pressing a scratchpad's key does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Run its command.
    Start,
    /// It's starting: nothing yet.
    Wait,
    /// Show it here and focus it.
    Show,
    /// It's shown here but not focused: focus it.
    Focus,
    /// It's shown on another monitor: bring it here and focus it.
    MoveHere,
    Hide,
}

impl<W, M: PartialEq> State<W, M> {
    /// What the key does now, on monitor `here`; `focused` is whether its
    /// window has the keyboard focus.
    pub fn step(&self, here: &M, focused: bool, now: Instant) -> Step {
        match self {
            State::Off => Step::Start,
            State::Starting { since, .. } if now.saturating_duration_since(*since) >= START_TIMEOUT => Step::Start,
            State::Starting { .. } => Step::Wait,
            State::Hidden(_) => Step::Show,
            State::Shown { on, .. } if on != here => Step::MoveHere,
            State::Shown { .. } if focused => Step::Hide,
            State::Shown { .. } => Step::Focus,
        }
    }

    /// Its window, if it has one.
    pub fn window(&self) -> Option<&W> {
        match self {
            State::Hidden(w) | State::Shown { window: w, .. } => Some(w),
            _ => None,
        }
    }

    /// Whether a new window from a client with process `pid` is the one
    /// this start is waiting for: that pid or a child of it, opened in
    /// time. `parent_of` gives a process's parent (`proc_parent`).
    pub fn claims(&self, pid: u32, now: Instant, parent_of: impl Fn(u32) -> Option<u32>) -> bool {
        match self {
            State::Starting { pid: started, since } => {
                now.saturating_duration_since(*since) < START_TIMEOUT && descends_from(pid, *started, parent_of)
            }
            _ => false,
        }
    }
}

/// Whether `pid` is `ancestor` or one of its descendants, walking up
/// through `parent_of`.
pub fn descends_from(mut pid: u32, ancestor: u32, parent_of: impl Fn(u32) -> Option<u32>) -> bool {
    // Bounded, so a loop in a faked table (or a pid reused mid-walk)
    // can't spin.
    for _ in 0..64 {
        if pid == ancestor {
            return true;
        }
        match parent_of(pid) {
            Some(p) if p != 0 && p != pid => pid = p,
            _ => return false,
        }
    }
    false
}

/// A process's parent, from `/proc/<pid>/stat`.
pub fn proc_parent(pid: u32) -> Option<u32> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    parent_from_stat(&stat)
}

/// The parent pid in a `/proc/<pid>/stat` line: the second field after
/// the command, which is in brackets and may hold spaces and brackets.
fn parent_from_stat(stat: &str) -> Option<u32> {
    let (_, rest) = stat.rsplit_once(')')?;
    rest.split_whitespace().nth(1)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pad(v: Value) -> Pad {
        Pad::parse(&v).expect("parses")
    }

    #[test]
    fn parses_a_full_pad() {
        let p = pad(json!({
            "id": "pad-1",
            "name": "Terminal",
            "command": "ghostty --gtk-single-instance=false",
            "keys": ["SUPER+GRAVE"],
            "size": [900, "40%"],
            "place": "centre",
            "hideOnBlur": true
        }));
        assert_eq!(
            p,
            Pad {
                id: "pad-1".into(),
                name: "Terminal".into(),
                command: "ghostty --gtk-single-instance=false".into(),
                keys: vec!["SUPER+GRAVE".into()],
                size: (Len::Px(900), Len::Fraction(0.4)),
                place: Place::Centre,
                hide_on_blur: true,
            }
        );
    }

    #[test]
    fn missing_fields_take_defaults() {
        let p = pad(json!({"id": "a", "command": "foot"}));
        assert_eq!(p.size, DEFAULT_SIZE);
        assert_eq!(p.place, Place::Top);
        assert!(!p.hide_on_blur);
        assert!(p.keys.is_empty());
        assert_eq!(pad(json!({"id": "a", "command": "foot", "size": [0, "50%"]})).size, DEFAULT_SIZE);
    }

    #[test]
    fn skips_pads_without_id_or_command_and_repeated_ids() {
        let pads = parse(&json!([
            {"id": "a", "command": "foot"},
            {"id": "b"},
            {"command": "foot"},
            {"id": "a", "command": "kitty"},
            {"id": "c", "command": "  "},
            "nope",
            {"id": "d", "command": "kitty"}
        ]));
        let ids: Vec<_> = pads.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["a", "d"]);
        assert_eq!(pads[0].command, "foot");
        assert!(parse(&json!({"id": "a"})).is_empty());
    }

    #[test]
    fn rect_is_centred_under_the_island_or_in_the_middle() {
        let area = Rect::new(0, 40, 1920, 1040);
        let top = pad(json!({"id": "a", "command": "x"}));
        assert_eq!(top.rect(area), Rect::new(384, 40, 1152, 520));
        let mid = pad(json!({"id": "a", "command": "x", "place": "centre", "size": [800, 600]}));
        assert_eq!(mid.rect(area), Rect::new(560, 260, 800, 600));
        // On a second monitor, and too big: cut to the area.
        let big = pad(json!({"id": "a", "command": "x", "size": [5000, 5000]}));
        assert_eq!(big.rect(Rect::new(1920, 0, 1280, 800)), Rect::new(1920, 0, 1280, 800));
        // An app that drew bigger than it was asked stays inside, from the top left.
        assert_eq!(big.origin(Rect::new(1920, 0, 1280, 800), (2000, 900)), (1920, 0));
        assert_eq!(mid.origin(area, (1000, 500)), (460, 310));
    }

    #[test]
    fn key_steps() {
        let now = Instant::now();
        let st = |s: State<u8, &str>, here, focused| s.step(&here, focused, now);
        assert_eq!(st(State::Off, "a", false), Step::Start);
        assert_eq!(st(State::Starting { pid: 1, since: now }, "a", false), Step::Wait);
        let stale = now - START_TIMEOUT;
        assert_eq!(st(State::Starting { pid: 1, since: stale }, "a", false), Step::Start);
        assert_eq!(st(State::Hidden(7), "a", false), Step::Show);
        assert_eq!(st(State::Shown { window: 7, on: "a" }, "a", false), Step::Focus);
        assert_eq!(st(State::Shown { window: 7, on: "a" }, "a", true), Step::Hide);
        assert_eq!(st(State::Shown { window: 7, on: "a" }, "b", true), Step::MoveHere);
        assert_eq!(st(State::Shown { window: 7, on: "a" }, "b", false), Step::MoveHere);
    }

    #[test]
    fn window_of_each_state() {
        assert_eq!(State::<u8, ()>::Off.window(), None);
        assert_eq!(State::<u8, ()>::Starting { pid: 1, since: Instant::now() }.window(), None);
        assert_eq!(State::<u8, ()>::Hidden(3).window(), Some(&3));
        assert_eq!(State::Shown { window: 3, on: () }.window(), Some(&3));
    }

    /// 1 → 100 (sh) → 200 (ghostty) → 300; 400 is unrelated.
    fn parents(pid: u32) -> Option<u32> {
        match pid {
            100 => Some(1),
            200 => Some(100),
            300 => Some(200),
            400 => Some(1),
            _ => None,
        }
    }

    #[test]
    fn descendants() {
        assert!(descends_from(100, 100, parents));
        assert!(descends_from(300, 100, parents));
        assert!(!descends_from(400, 100, parents));
        assert!(!descends_from(100, 200, parents));
        // A loop in the table ends.
        assert!(!descends_from(5, 9, |p| Some(if p == 5 { 6 } else { 5 })));
    }

    #[test]
    fn claims_only_the_started_process_tree_in_time() {
        let now = Instant::now();
        let starting: State<u8, ()> = State::Starting { pid: 100, since: now };
        assert!(starting.claims(300, now, parents));
        assert!(!starting.claims(400, now, parents));
        assert!(!starting.claims(300, now + START_TIMEOUT, parents));
        assert!(!State::<u8, ()>::Hidden(1).claims(100, now, parents));
    }

    #[test]
    fn parent_from_stat_lines() {
        assert_eq!(parent_from_stat("1234 (ghostty) S 1000 1234 1234 0 -1"), Some(1000));
        // A command with spaces and brackets.
        assert_eq!(parent_from_stat("55 (my (odd) app) R 7 55 55"), Some(7));
        assert_eq!(parent_from_stat("garbage"), None);
        assert_eq!(proc_parent(std::process::id()), Some(std::os::unix::process::parent_id()));
    }
}
