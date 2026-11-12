//! Window rules (`windows.rules`): what to do with a window when it opens,
//! matched by its app id and title.
//!
//! Every rule that matches applies, in list order; when two set the same
//! thing, the later one wins. The result is one [`Actions`] per window,
//! worked out once when it opens. Pure: no compositor state, so it's tested
//! on its own.

use regex::Regex;
use serde_json::Value;

use crate::layout::Rect;

/// A length: pixels, or a fraction of the monitor's usable area.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Len {
    Px(i32),
    /// 0–1 (`"60%"` is 0.6).
    Fraction(f32),
}

impl Len {
    /// `900` or `"60%"`; `min` is the smallest pixel count allowed.
    fn parse(v: &Value, min: i64) -> Option<Len> {
        if let Some(n) = v.as_i64() {
            return (n >= min).then(|| Len::Px(n.min(i64::from(i32::MAX)) as i32));
        }
        let pct: f32 = v.as_str()?.trim().strip_suffix('%')?.trim().parse().ok()?;
        (0.0..=100.0).contains(&pct).then_some(Len::Fraction(pct / 100.0))
    }

    /// In pixels, out of `total`.
    pub fn of(self, total: i32) -> i32 {
        match self {
            Len::Px(p) => p,
            Len::Fraction(f) => (total as f32 * f).round() as i32,
        }
    }
}

/// Where a floating window opens in the monitor's usable area.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Position {
    Centre,
    /// From the area's top left.
    At(Len, Len),
}

/// What the rules say to do with one window. `None` is "not set": the
/// window does what it would without rules.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Actions {
    /// 1-based.
    pub workspace: Option<u8>,
    /// An EDID monitor id.
    pub monitor: Option<String>,
    /// `true` keeps it out of the tiles; `false` tiles it if it can be.
    pub float: Option<bool>,
    pub size: Option<(Len, Len)>,
    pub position: Option<Position>,
    pub maximized: Option<bool>,
    pub fullscreen: Option<bool>,
    /// (focused, unfocused), each 0.1–1.
    pub opacity: Option<(f32, f32)>,
    pub on_top: Option<bool>,
    /// 1-based zone of the snap grid it opens on.
    pub zone: Option<u8>,
}

impl Actions {
    fn parse(o: &serde_json::Map<String, Value>) -> Actions {
        let bool_of = |k: &str| o.get(k).and_then(Value::as_bool);
        let pair = |k: &str| o.get(k).and_then(Value::as_array).filter(|a| a.len() == 2);
        let small = |k: &str| {
            o.get(k).and_then(Value::as_u64).filter(|n| (1..=u64::from(u8::MAX)).contains(n)).map(|n| n as u8)
        };
        let opacity = |v: &Value| v.as_f64().map(|f| (f as f32).clamp(0.1, 1.0));
        Actions {
            workspace: small("workspace"),
            monitor: o.get("monitor").and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_owned),
            float: bool_of("float"),
            size: pair("size").and_then(|a| Some((Len::parse(&a[0], 1)?, Len::parse(&a[1], 1)?))),
            position: match o.get("position") {
                Some(Value::String(s)) if matches!(s.as_str(), "centre" | "center") => Some(Position::Centre),
                Some(Value::Array(a)) if a.len() == 2 => {
                    Len::parse(&a[0], 0).zip(Len::parse(&a[1], 0)).map(|(x, y)| Position::At(x, y))
                }
                _ => None,
            },
            maximized: bool_of("maximized"),
            fullscreen: bool_of("fullscreen"),
            opacity: pair("opacity").and_then(|a| opacity(&a[0]).zip(opacity(&a[1]))),
            on_top: bool_of("onTop"),
            zone: small("zone"),
        }
    }

    /// Takes every action `later` sets, over this one's.
    fn merge(&mut self, later: &Actions) {
        fn over<T: Clone>(a: &mut Option<T>, b: &Option<T>) {
            if b.is_some() {
                a.clone_from(b);
            }
        }
        over(&mut self.workspace, &later.workspace);
        over(&mut self.monitor, &later.monitor);
        over(&mut self.float, &later.float);
        over(&mut self.size, &later.size);
        over(&mut self.position, &later.position);
        over(&mut self.maximized, &later.maximized);
        over(&mut self.fullscreen, &later.fullscreen);
        over(&mut self.opacity, &later.opacity);
        over(&mut self.on_top, &later.on_top);
        over(&mut self.zone, &later.zone);
    }

    /// Whether it sets nothing.
    pub fn is_empty(&self) -> bool {
        *self == Actions::default()
    }

    /// The size a floating window is told to open at in the usable
    /// `area`, cut to fit it. `None` lets the app choose.
    pub fn size_in(&self, area: Rect) -> Option<(i32, i32)> {
        let (w, h) = self.size?;
        Some((w.of(area.w).clamp(1, area.w.max(1)), h.of(area.h).clamp(1, area.h.max(1))))
    }

    /// Where a floating window of `size` (the size it drew) goes in the
    /// usable `area`: its top left, kept inside the area. `None` leaves it
    /// where it would open.
    pub fn position_in(&self, area: Rect, size: (i32, i32)) -> Option<(i32, i32)> {
        let (w, h) = size;
        let (x, y) = match self.position? {
            Position::Centre => (area.x + (area.w - w) / 2, area.y + (area.h - h) / 2),
            Position::At(x, y) => (area.x + x.of(area.w), area.y + y.of(area.h)),
        };
        Some((x.clamp(area.x, (area.x + area.w - w).max(area.x)), y.clamp(area.y, (area.y + area.h - h).max(area.y))))
    }
}

/// One rule: what it matches and what it does.
#[derive(Debug, Clone)]
struct Rule {
    app: Option<Regex>,
    title: Option<Regex>,
    actions: Actions,
}

impl Rule {
    fn matches(&self, app: &str, title: &str) -> bool {
        self.app.as_ref().is_none_or(|r| r.is_match(app)) && self.title.as_ref().is_none_or(|r| r.is_match(title))
    }
}

/// The rules, in order.
#[derive(Debug, Clone, Default)]
pub struct Rules(Vec<Rule>);

impl Rules {
    /// Reads `windows.rules`. A rule with no pattern, or one that doesn't
    /// compile, is skipped (and logged); the rest still apply.
    pub fn parse(v: &Value) -> Rules {
        let Some(list) = v.as_array() else { return Rules::default() };
        let rules = list
            .iter()
            .enumerate()
            .filter_map(|(i, r)| {
                let o = r.as_object()?;
                let pattern = |k: &str| -> Result<Option<Regex>, regex::Error> {
                    match o.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()) {
                        None => Ok(None),
                        Some(p) => Regex::new(p).map(Some),
                    }
                };
                match (pattern("app"), pattern("title")) {
                    (Ok(None), Ok(None)) => {
                        tracing::warn!(rule = i + 1, "skipping a window rule with no app id or title");
                        None
                    }
                    (Ok(app), Ok(title)) => Some(Rule { app, title, actions: Actions::parse(o) }),
                    (Err(e), _) | (_, Err(e)) => {
                        tracing::warn!(rule = i + 1, %e, "skipping a window rule that doesn't compile");
                        None
                    }
                }
            })
            .collect();
        Rules(rules)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// What every rule matching this app id and title says, later rules
    /// winning. A window without an app id or title matches as "".
    pub fn resolve(&self, app: &str, title: &str) -> Actions {
        let mut out = Actions::default();
        for r in self.0.iter().filter(|r| r.matches(app, title)) {
            out.merge(&r.actions);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rules(v: Value) -> Rules {
        Rules::parse(&v)
    }

    #[test]
    fn matches_by_app_and_title() {
        let r = rules(json!([
            {"app": "^firefox$", "workspace": 2},
            {"title": "Picture-in-Picture", "onTop": true},
            {"app": "ptyxis", "title": "^ssh", "workspace": 5},
        ]));
        assert_eq!(r.resolve("firefox", "Mozilla Firefox").workspace, Some(2));
        assert_eq!(r.resolve("firefox-dev", "x"), Actions::default(), "anchored");
        assert_eq!(r.resolve("firefox", "Picture-in-Picture").on_top, Some(true));
        assert_eq!(r.resolve("org.gnome.Ptyxis", "ssh host").workspace, None, "case matters");
        assert_eq!(r.resolve("ptyxis", "ssh host").workspace, Some(5));
        assert_eq!(r.resolve("ptyxis", "~").workspace, None, "both must match");
    }

    #[test]
    fn later_rules_win_per_action() {
        let r = rules(json!([
            {"app": "code", "workspace": 3, "float": true},
            {"app": ".", "workspace": 1},
            {"app": "code", "float": false},
        ]));
        let a = r.resolve("code", "");
        assert_eq!(a.workspace, Some(1));
        assert_eq!(a.float, Some(false));
        assert_eq!(r.resolve("other", "").float, None);
    }

    #[test]
    fn bad_rules_are_skipped() {
        let r = rules(json!([
            {"workspace": 2},
            {"app": "", "title": "", "workspace": 2},
            {"app": "(", "workspace": 3},
            "nope",
            {"app": "a", "workspace": 4},
        ]));
        assert_eq!(r.0.len(), 1);
        assert_eq!(r.resolve("a", "").workspace, Some(4));
        assert!(rules(json!({"app": "a"})).is_empty());
        assert!(rules(Value::Null).is_empty());
    }

    #[test]
    fn parses_every_action() {
        let r = rules(json!([{
            "app": "a",
            "workspace": 3,
            "monitor": "DEL-41A6-1",
            "float": true,
            "size": [900, "60%"],
            "position": "centre",
            "maximized": false,
            "fullscreen": true,
            "opacity": [1.0, 0.85],
            "onTop": true,
            "zone": 2,
        }]));
        assert_eq!(
            r.resolve("a", ""),
            Actions {
                workspace: Some(3),
                monitor: Some("DEL-41A6-1".into()),
                float: Some(true),
                size: Some((Len::Px(900), Len::Fraction(0.6))),
                position: Some(Position::Centre),
                maximized: Some(false),
                fullscreen: Some(true),
                opacity: Some((1.0, 0.85)),
                on_top: Some(true),
                zone: Some(2),
            }
        );
    }

    #[test]
    fn ignores_values_out_of_range() {
        let r = rules(json!([{
            "app": "a",
            "workspace": 0,
            "monitor": "",
            "size": [0, "50%"],
            "position": [10, "120%"],
            "opacity": [0.0, 2],
            "zone": 300,
        }]));
        let a = r.resolve("a", "");
        assert_eq!(a.workspace, None);
        assert_eq!(a.monitor, None);
        assert_eq!(a.size, None);
        assert_eq!(a.position, None);
        assert_eq!(a.opacity, Some((0.1, 1.0)), "opacity is clamped, not dropped");
        assert_eq!(a.zone, None);
        assert_eq!(rules(json!([{"app": "a", "position": "middle"}])).resolve("a", "").position, None);
        assert!(rules(json!([{"app": "a", "size": [1]}])).resolve("a", "").is_empty());
    }

    #[test]
    fn sizes_floating_windows() {
        let area = Rect::new(100, 40, 1000, 800);
        let with = |v: Value| Rules::parse(&json!([v])).resolve("a", "");
        assert_eq!(with(json!({"app": "a"})).size_in(area), None);
        assert_eq!(with(json!({"app": "a", "size": ["50%", 400]})).size_in(area), Some((500, 400)));
        assert_eq!(with(json!({"app": "a", "size": [5000, "100%"]})).size_in(area), Some((1000, 800)), "cut to fit");
    }

    #[test]
    fn positions_floating_windows() {
        let area = Rect::new(100, 40, 1000, 800);
        let with = |v: Value| Rules::parse(&json!([v])).resolve("a", "");
        assert_eq!(with(json!({"app": "a"})).position_in(area, (300, 200)), None);
        assert_eq!(with(json!({"app": "a", "position": "centre"})).position_in(area, (300, 200)), Some((450, 340)));
        assert_eq!(with(json!({"app": "a", "position": "center"})).position_in(area, (300, 200)), Some((450, 340)));
        // From the area's top left, in pixels or fractions.
        assert_eq!(with(json!({"app": "a", "position": [20, "25%"]})).position_in(area, (300, 200)), Some((120, 240)));
        // Past the right and bottom: moved back in; too big: the top left.
        assert_eq!(with(json!({"app": "a", "position": ["90%", 790]})).position_in(area, (300, 200)), Some((800, 640)));
        assert_eq!(with(json!({"app": "a", "position": [50, 50]})).position_in(area, (2000, 2000)), Some((100, 40)));
    }
}
