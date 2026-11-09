//! Turns the user's display settings (the "displays" setting, the same shape
//! as the daemon's config.Displays) and the monitors actually connected into
//! a plan for each output: on or off, mode, scale, rotation, position.
//!
//! - Per-monitor preferences (mode, scale, rotation, VRR) follow the monitor
//!   by its stable ID, whichever port it's on.
//! - Positions come from the first profile whose monitors are exactly the
//!   connected set (users order profiles by priority); with no matching
//!   profile, monitors are laid out left to right.
//! - At least one monitor always stays on.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Displays {
    #[serde(default)]
    pub monitor: Vec<Monitor>,
    #[serde(default)]
    pub profile: Vec<Profile>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Monitor {
    pub id: String,
    #[serde(default)]
    pub label: String,
    /// "2560x1440@165", "2560x1440", "" or "preferred".
    #[serde(default)]
    pub mode: String,
    /// 0 means 1.
    #[serde(default)]
    pub scale: f64,
    /// normal | 90 | 180 | 270 | flipped | flipped-90 | flipped-180 | flipped-270
    #[serde(default)]
    pub transform: String,
    /// "", auto, on, off
    #[serde(default)]
    pub vrr: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub output: Vec<ProfileOutput>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProfileOutput {
    pub id: String,
    pub position: [i32; 2],
    #[serde(default)]
    pub disabled: bool,
}

/// A mode a monitor offers. Refresh in millihertz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeInfo {
    pub width: i32,
    pub height: i32,
    pub refresh: i32,
    pub preferred: bool,
}

/// A connected monitor, as the backend sees it.
#[derive(Debug, Clone)]
pub struct Connected {
    pub id: String,
    pub modes: Vec<ModeInfo>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub id: String,
    pub enabled: bool,
    pub mode: Option<ModeInfo>,
    pub scale: f64,
    pub transform: String,
    pub position: (i32, i32),
    pub vrr: bool,
    /// Which profile placed it, if any.
    pub profile: Option<String>,
}

/// "2560x1440@165" -> (2560, 1440, Some(165000)); "" / "preferred" -> None.
pub fn parse_mode(s: &str) -> Option<(i32, i32, Option<i32>)> {
    let s = s.trim();
    if s.is_empty() || s == "preferred" {
        return None;
    }
    let (size, hz) = match s.split_once('@') {
        Some((a, b)) => (a, Some(b)),
        None => (s, None),
    };
    let (w, h) = size.split_once('x')?;
    let refresh = match hz {
        Some(hz) => Some((hz.parse::<f64>().ok()? * 1000.0).round() as i32),
        None => None,
    };
    Some((w.parse().ok()?, h.parse().ok()?, refresh))
}

/// The available mode closest to what the user asked for: the same size at
/// the nearest refresh (the highest when none was asked); otherwise the
/// monitor's preferred mode.
pub fn pick_mode(modes: &[ModeInfo], wanted: &str) -> Option<ModeInfo> {
    let preferred = modes.iter().find(|m| m.preferred).or(modes.first()).copied();
    let Some((w, h, hz)) = parse_mode(wanted) else { return preferred };
    let same: Vec<_> = modes.iter().filter(|m| m.width == w && m.height == h).collect();
    let best = match hz {
        Some(hz) => same.iter().min_by_key(|m| (m.refresh - hz).abs()),
        None => same.iter().max_by_key(|m| m.refresh),
    };
    best.map(|m| **m).or(preferred)
}

pub fn swaps_axes(t: &str) -> bool {
    matches!(t, "90" | "270" | "flipped-90" | "flipped-270")
}

/// The size a monitor takes in the shared (logical) space, as the daemon's
/// monitor.LogicalSize computes it.
pub fn logical_size(mode: ModeInfo, scale: f64, transform: &str) -> (i32, i32) {
    let s = if scale <= 0.0 { 1.0 } else { scale };
    let (w, h) = ((mode.width as f64 / s).round() as i32, (mode.height as f64 / s).round() as i32);
    if swaps_axes(transform) {
        (h, w)
    } else {
        (w, h)
    }
}

impl Displays {
    pub fn monitor(&self, id: &str) -> Monitor {
        self.monitor.iter().find(|m| m.id == id).cloned().unwrap_or(Monitor { id: id.into(), ..Default::default() })
    }

    pub fn match_profile(&self, connected: &[String]) -> Option<&Profile> {
        let mut want = connected.to_vec();
        want.sort();
        self.profile.iter().find(|p| {
            let mut have: Vec<_> = p.output.iter().map(|o| o.id.clone()).collect();
            have.sort();
            have == want
        })
    }
}

/// The plan for every connected monitor, in the given order (which is also
/// the left-to-right order when no profile matches).
pub fn plan(cfg: &Displays, connected: &[Connected]) -> Vec<Plan> {
    let ids: Vec<String> = connected.iter().map(|c| c.id.clone()).collect();
    let profile = cfg.match_profile(&ids);
    let mut x = 0;
    let mut plans: Vec<Plan> = connected
        .iter()
        .map(|c| {
            let m = cfg.monitor(&c.id);
            let mode = pick_mode(&c.modes, &m.mode);
            let scale = if m.scale > 0.0 { m.scale } else { 1.0 };
            let transform = if m.transform.is_empty() { "normal".to_string() } else { m.transform.clone() };
            let placed = profile.and_then(|p| p.output.iter().find(|o| o.id == c.id));
            let (position, enabled) = match placed {
                Some(o) => ((o.position[0], o.position[1]), !o.disabled),
                None => {
                    let at = (x, 0);
                    if let Some(mode) = mode {
                        x += logical_size(mode, scale, &transform).0;
                    }
                    (at, true)
                }
            };
            Plan {
                id: c.id.clone(),
                enabled: enabled && mode.is_some(),
                mode,
                scale,
                transform,
                position,
                vrr: m.vrr == "on",
                profile: placed.and(profile).map(|p| p.name.clone()),
            }
        })
        .collect();
    // Never every monitor off: keep the first that can show something.
    if !plans.iter().any(|p| p.enabled) {
        if let Some(p) = plans.iter_mut().find(|p| p.mode.is_some()) {
            p.enabled = true;
        }
    }
    plans
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(w: i32, h: i32, hz: i32, preferred: bool) -> ModeInfo {
        ModeInfo { width: w, height: h, refresh: hz * 1000, preferred }
    }
    fn dell() -> Connected {
        Connected {
            id: "edid:0d4384bf042b".into(),
            modes: vec![
                mode(3840, 2160, 60, true),
                mode(2560, 1440, 165, false),
                mode(2560, 1440, 60, false),
                mode(1920, 1080, 60, false),
            ],
        }
    }
    fn panel() -> Connected {
        Connected { id: "builtin".into(), modes: vec![mode(2560, 1600, 120, true), mode(2560, 1600, 60, false)] }
    }

    #[test]
    fn modes_are_parsed_and_matched_to_what_the_monitor_offers() {
        assert_eq!(parse_mode("2560x1440@165"), Some((2560, 1440, Some(165000))));
        assert_eq!(parse_mode("1920x1080@59.94"), Some((1920, 1080, Some(59940))));
        assert_eq!(parse_mode("preferred"), None);
        assert_eq!(parse_mode("big"), None);
        let modes = dell().modes;
        assert_eq!(pick_mode(&modes, "2560x1440@144").unwrap().refresh, 165000, "nearest refresh");
        assert_eq!(pick_mode(&modes, "2560x1440").unwrap().refresh, 165000, "highest when unspecified");
        assert_eq!(pick_mode(&modes, "1280x720").unwrap().width, 3840, "unavailable: preferred");
        assert_eq!(pick_mode(&modes, "").unwrap().width, 3840);
        assert_eq!(pick_mode(&[], ""), None);
    }

    #[test]
    fn logical_size_matches_the_daemon() {
        assert_eq!(logical_size(mode(2560, 1440, 60, true), 1.25, "normal"), (2048, 1152));
        assert_eq!(logical_size(mode(2560, 1440, 60, true), 0.0, "90"), (1440, 2560));
        assert_eq!(logical_size(mode(3840, 2160, 60, true), 1.5, "flipped-270"), (1440, 2560));
    }

    #[test]
    fn without_a_profile_monitors_go_left_to_right_with_their_preferences() {
        let cfg: Displays = serde_json::from_str(
            r#"{"monitor":[{"id":"builtin","scale":1.6},{"id":"edid:0d4384bf042b","mode":"2560x1440@165","transform":"90","vrr":"on"}],"profile":[]}"#,
        )
        .unwrap();
        let p = plan(&cfg, &[panel(), dell()]);
        assert_eq!((p[0].position, p[0].scale, p[0].mode.unwrap().refresh), ((0, 0), 1.6, 120000));
        // 2560/1.6 = 1600 wide, so the Dell starts there; rotated, it's 1440 wide.
        assert_eq!((p[1].position, p[1].transform.as_str(), p[1].vrr), ((1600, 0), "90", true));
        assert_eq!(p[1].mode.unwrap().refresh, 165000);
        assert!(p.iter().all(|p| p.enabled && p.profile.is_none()));
    }

    #[test]
    fn the_first_profile_matching_exactly_the_connected_monitors_places_them() {
        let cfg: Displays = serde_json::from_str(
            r#"{"profile":[
                {"name":"Desk + other","output":[{"id":"builtin","position":[0,0]},{"id":"edid:0d4384bf042b","position":[0,0]},{"id":"edid:ffffffffffff","position":[0,0]}]},
                {"name":"Desk","output":[{"id":"edid:0d4384bf042b","position":[0,0]},{"id":"builtin","position":[3840,1080],"disabled":true}]},
                {"name":"Later duplicate","output":[{"id":"builtin","position":[9,9]},{"id":"edid:0d4384bf042b","position":[9,9]}]}
            ]}"#,
        )
        .unwrap();
        let p = plan(&cfg, &[panel(), dell()]);
        assert_eq!(p[0].profile.as_deref(), Some("Desk"));
        assert_eq!((p[0].position, p[0].enabled), ((3840, 1080), false), "lid closed at the desk");
        assert_eq!((p[1].position, p[1].enabled), ((0, 0), true));
    }

    #[test]
    fn at_least_one_monitor_stays_on() {
        let cfg: Displays = serde_json::from_str(
            r#"{"profile":[{"name":"All off","output":[{"id":"builtin","position":[0,0],"disabled":true}]}]}"#,
        )
        .unwrap();
        let p = plan(&cfg, &[panel()]);
        assert!(p[0].enabled, "turning off the only monitor would leave a black screen");
    }

    #[test]
    fn reads_what_the_daemon_stores() {
        // The shape config.Displays marshals to (omitempty fields left out).
        let cfg: Displays = serde_json::from_str(
            r#"{"monitor":[{"id":"builtin"}],"profile":[{"output":[{"id":"builtin","position":[0,0]}]}]}"#,
        )
        .unwrap();
        assert_eq!(cfg.monitor[0].scale, 0.0);
        assert_eq!(cfg.profile[0].name, "");
    }
}
