//! Keyboard shortcuts: parsing ("SUPER+SHIFT+S"), the default table, and
//! lookup. Pure logic, no compositor state, so it is fully unit-tested.

use std::fmt;

use smithay::input::keyboard::{keysyms, xkb, Keysym, ModifiersState};

/// The modifiers that take part in shortcuts. Caps/Num lock are ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Mods {
    pub logo: bool,
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
}

impl Mods {
    pub const NONE: Mods = Mods { logo: false, alt: false, ctrl: false, shift: false };

    fn union(self, o: Mods) -> Mods {
        Mods {
            logo: self.logo || o.logo,
            alt: self.alt || o.alt,
            ctrl: self.ctrl || o.ctrl,
            shift: self.shift || o.shift,
        }
    }
}

impl From<&ModifiersState> for Mods {
    fn from(m: &ModifiersState) -> Self {
        Mods { logo: m.logo, alt: m.alt, ctrl: m.ctrl, shift: m.shift }
    }
}

/// What a shortcut does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    // Handled by the shell (QuickShell) once IPC exists.
    AppLauncher,
    AppSwitcher,
    WindowSwitcher,
    QuickSettings,
    /// Lock the screen (the shell shows its lock screen).
    Lock,
    /// Show each output's number on it (the shell draws it).
    IdentifyOutputs,
    Notifications,
    ScreenshotRegion,
    // Handled by the compositor.
    /// PrtSc: capture the whole screen straight away.
    ScreenshotScreen,
    CloseWindow,
    ToggleFullscreen,
    /// Switch to workspace N (1-based, as shown to the user).
    Workspace(u8),
    /// Move the focused window to workspace N (1-based).
    MoveToWorkspace(u8),
    Spawn(String),
    /// Focus a window by its IPC id (switching workspace if needed).
    FocusWindow(u64),
    /// Focus an app's most recent window, or cycle if it already has focus.
    FocusApp(String),
    /// Delete a screenshot this compositor saved (requested by the shell).
    DeleteScreenshot(String),
    /// Night Light on/off and its warmth (from the shell's quick settings).
    NightLight(crate::night_light::NightLight),
    /// Clipboard and screenshot-tray requests from the shell.
    Shell(ShellRequest),
    /// Ctrl+Alt+Fn: switch to console n (real hardware only).
    SwitchVt(i32),
    /// The screenshot tool's capture (region, pointer, save or copy).
    Capture(crate::screenshot::Shot),
    /// The next keyboard layout (SUPER+SPACE).
    NextLayout,
    /// Volume, brightness and media keys, done by the shell; the name is
    /// its wire name (e.g. "volume_up").
    Media(&'static str),
    Quit,
}

impl Action {
    /// True for actions the shell UI implements rather than the compositor.
    pub fn is_shell(&self) -> bool {
        matches!(
            self,
            Action::AppLauncher
                | Action::QuickSettings
                | Action::Lock
                | Action::IdentifyOutputs
                | Action::Notifications
                | Action::ScreenshotRegion
                | Action::Media(_)
        )
    }
}

/// Requests only the shell sends (never bound to keys).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellRequest {
    ClipboardList,
    ClipboardPaste(u64),
    ClipboardPrepareDrag(u64),
    ClipboardDelete(u64),
    ClipboardClear,
    ScreenshotList,
    ScreenshotCopy(String),
    /// Re-send the window list (with positions) now.
    WindowsList,
}

/// A key plus modifiers. `key` is stored lower-cased so "S" and "s" match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Combo {
    pub mods: Mods,
    pub key: Keysym,
}

impl Combo {
    /// A combo that is just the SUPER key on its own, triggered on release.
    pub fn is_logo_tap(&self) -> bool {
        self.mods == Mods::NONE && is_logo_key(self.key)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    UnknownModifier(String),
    DuplicateModifier(String),
    UnknownKey(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Empty => write!(f, "empty shortcut"),
            ParseError::UnknownModifier(m) => write!(f, "unknown modifier {m:?} (use SUPER, ALT, CTRL or SHIFT)"),
            ParseError::DuplicateModifier(m) => write!(f, "modifier {m:?} listed twice"),
            ParseError::UnknownKey(k) => write!(f, "unknown key {k:?}"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parses "SUPER+SHIFT+S", "ALT+TAB", "PRTSC" or "SUPER". Case-insensitive.
/// The last token is the key; everything before it must be a modifier.
pub fn parse_combo(s: &str) -> Result<Combo, ParseError> {
    let tokens: Vec<&str> = s.split('+').map(str::trim).collect();
    if tokens.iter().any(|t| t.is_empty()) {
        return Err(ParseError::Empty);
    }
    let (key_tok, mod_toks) = tokens.split_last().ok_or(ParseError::Empty)?;

    let mut mods = Mods::NONE;
    for tok in mod_toks {
        let flag = match tok.to_ascii_uppercase().as_str() {
            "SUPER" | "LOGO" | "WIN" | "MOD4" => &mut mods.logo,
            "ALT" | "MOD1" => &mut mods.alt,
            "CTRL" | "CONTROL" => &mut mods.ctrl,
            "SHIFT" => &mut mods.shift,
            _ => return Err(ParseError::UnknownModifier(tok.to_string())),
        };
        if *flag {
            return Err(ParseError::DuplicateModifier(tok.to_string()));
        }
        *flag = true;
    }
    Ok(Combo { mods, key: parse_key(key_tok)? })
}

fn parse_key(name: &str) -> Result<Keysym, ParseError> {
    let alias = match name.to_ascii_uppercase().as_str() {
        "SUPER" | "LOGO" | "WIN" => Some(Keysym::from(keysyms::KEY_Super_L)),
        "PRTSC" | "PRTSCN" | "PRINT" | "PRINTSCREEN" => Some(Keysym::from(keysyms::KEY_Print)),
        "ENTER" | "RETURN" => Some(Keysym::from(keysyms::KEY_Return)),
        "ESC" | "ESCAPE" => Some(Keysym::from(keysyms::KEY_Escape)),
        "TAB" => Some(Keysym::from(keysyms::KEY_Tab)),
        "SPACE" => Some(Keysym::from(keysyms::KEY_space)),
        "BACKSPACE" => Some(Keysym::from(keysyms::KEY_BackSpace)),
        _ => None,
    };
    if let Some(k) = alias {
        return Ok(k);
    }
    let sym = xkb::keysym_from_name(name, xkb::KEYSYM_CASE_INSENSITIVE);
    if sym.raw() == keysyms::KEY_NoSymbol {
        return Err(ParseError::UnknownKey(name.to_string()));
    }
    Ok(lower(sym))
}

/// Lower-cases Latin letters so shortcuts match regardless of Shift/Caps.
pub fn lower(sym: Keysym) -> Keysym {
    let raw = sym.raw();
    if (keysyms::KEY_A..=keysyms::KEY_Z).contains(&raw) {
        Keysym::from(raw + (keysyms::KEY_a - keysyms::KEY_A))
    } else {
        sym
    }
}

pub fn is_logo_key(sym: Keysym) -> bool {
    matches!(sym.raw(), keysyms::KEY_Super_L | keysyms::KEY_Super_R)
}

/// "SUPER+1-9" is nine combos (1 to 9); anything else is one.
pub fn expand(s: &str) -> Result<Vec<Combo>, ParseError> {
    match s.rsplit_once('+') {
        Some((mods, "1-9")) => (1..=9).map(|d| parse_combo(&format!("{mods}+{d}"))).collect(),
        None if s == "1-9" => Err(ParseError::UnknownKey(s.into())),
        _ => parse_combo(s).map(|c| vec![c]),
    }
}

/// A shortcut you added: runs `command`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CustomShortcut {
    pub id: String,
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub keys: Vec<String>,
}

/// One built-in shortcut: a stable id (the settings key), what it's called
/// and grouped under on the Keyboard & Shortcuts page, and its default keys.
#[derive(Debug, Clone, Copy)]
pub struct Def {
    pub id: &'static str,
    pub name: &'static str,
    pub group: &'static str,
    pub defaults: &'static [&'static str],
    /// Bound to "MODS+1-9": workspace n for key n.
    pub range: bool,
}

impl Def {
    fn action(&self, terminal: &str, n: u8) -> Action {
        match self.id {
            "launcher" => Action::AppLauncher,
            "app-switcher" => Action::AppSwitcher,
            "window-switcher" => Action::WindowSwitcher,
            "quick-settings" => Action::QuickSettings,
            "notifications" => Action::Notifications,
            "lock" => Action::Lock,
            "screenshot-region" => Action::ScreenshotRegion,
            "screenshot-screen" => Action::ScreenshotScreen,
            "terminal" => Action::Spawn(terminal.to_string()),
            "switch-layout" => Action::NextLayout,
            "log-out" => Action::Quit,
            "close-window" => Action::CloseWindow,
            "fullscreen" => Action::ToggleFullscreen,
            "workspace" => Action::Workspace(n),
            "move-to-workspace" => Action::MoveToWorkspace(n),
            "volume-up" => Action::Media("volume_up"),
            "volume-down" => Action::Media("volume_down"),
            "volume-mute" => Action::Media("volume_mute"),
            "mic-mute" => Action::Media("mic_mute"),
            "brightness-up" => Action::Media("brightness_up"),
            "brightness-down" => Action::Media("brightness_down"),
            "play-pause" => Action::Media("media_play_pause"),
            "next-track" => Action::Media("media_next"),
            "previous-track" => Action::Media("media_previous"),
            other => unreachable!("catalog id {other} has no action"),
        }
    }
}

const fn def(id: &'static str, name: &'static str, group: &'static str, defaults: &'static [&'static str]) -> Def {
    Def { id, name, group, defaults, range: false }
}

/// Every built-in shortcut, in the order the settings page lists them.
pub const CATALOG: &[Def] = &[
    def("launcher", "App launcher", "Shell", &["SUPER"]),
    def("app-switcher", "Application switcher", "Windows", &["ALT+TAB"]),
    def("window-switcher", "Window switcher", "Windows", &["SUPER+ALT+TAB"]),
    def("quick-settings", "Quick settings", "Shell", &["SUPER+S"]),
    def("notifications", "Notifications", "Shell", &["SUPER+N"]),
    def("fullscreen", "Toggle fullscreen", "Windows", &["SUPER+F"]),
    def("close-window", "Close window", "Windows", &["SUPER+Q"]),
    Def { range: true, ..def("workspace", "Switch to workspace 1–9", "Workspaces", &["SUPER+1-9"]) },
    Def {
        range: true, ..def("move-to-workspace", "Move window to workspace 1–9", "Workspaces", &["SUPER+SHIFT+1-9"])
    },
    def("screenshot-region", "Screenshot region", "Shell", &["SUPER+SHIFT+S"]),
    def("screenshot-screen", "Screenshot screen", "Shell", &["PRTSC"]),
    def("terminal", "Open terminal", "Shell", &["SUPER+RETURN"]),
    def("lock", "Lock screen", "Shell", &["SUPER+L"]),
    def("switch-layout", "Next keyboard layout", "Shell", &["SUPER+SPACE"]),
    def("log-out", "Log out", "Shell", &["CTRL+ALT+BACKSPACE"]),
    def("volume-up", "Volume up", "Media", &["XF86AudioRaiseVolume"]),
    def("volume-down", "Volume down", "Media", &["XF86AudioLowerVolume"]),
    def("volume-mute", "Mute", "Media", &["XF86AudioMute"]),
    def("mic-mute", "Mute microphone", "Media", &["XF86AudioMicMute"]),
    def("brightness-up", "Brightness up", "Media", &["XF86MonBrightnessUp"]),
    def("brightness-down", "Brightness down", "Media", &["XF86MonBrightnessDown"]),
    def("play-pause", "Play / pause", "Media", &["XF86AudioPlay"]),
    def("next-track", "Next track", "Media", &["XF86AudioNext"]),
    def("previous-track", "Previous track", "Media", &["XF86AudioPrev"]),
];

/// The shortcut table.
#[derive(Debug, Clone)]
pub struct Bindings {
    entries: Vec<(Combo, Action)>,
}

impl Bindings {
    pub fn new(entries: Vec<(Combo, Action)>) -> Self {
        Bindings { entries }
    }

    /// WhiteRose's default shortcuts (see the Keyboard & Shortcuts design).
    pub fn defaults(terminal: &str) -> Self {
        Bindings::build(terminal, &Default::default(), &[]).0
    }

    /// The shortcut table from the catalog's defaults, the user's changes
    /// (`overrides`: id -> combos; an empty list turns one off) and their
    /// own shortcuts. Combos that don't parse are skipped and reported.
    pub fn build(
        terminal: &str,
        overrides: &std::collections::HashMap<String, Vec<String>>,
        custom: &[CustomShortcut],
    ) -> (Self, Vec<String>) {
        let mut entries = Vec::new();
        let mut errors = Vec::new();
        for def in CATALOG {
            let combos: Vec<String> = match overrides.get(def.id) {
                Some(c) => c.clone(),
                None => def.defaults.iter().map(|s| s.to_string()).collect(),
            };
            for c in combos {
                match expand(&c) {
                    Ok(list) if def.range => {
                        if list.len() != 9 {
                            errors.push(format!("{}: {c:?} needs the 1-9 key", def.id));
                            continue;
                        }
                        for (n, combo) in list.into_iter().enumerate() {
                            entries.push((combo, def.action(terminal, n as u8 + 1)));
                        }
                    }
                    Ok(list) if list.len() == 1 => entries.push((list[0], def.action(terminal, 0))),
                    Ok(_) => errors.push(format!("{}: {c:?} can't use 1-9", def.id)),
                    Err(e) => errors.push(format!("{}: {c:?}: {e}", def.id)),
                }
            }
        }
        for c in custom {
            for k in &c.keys {
                match expand(k) {
                    Ok(list) if list.len() == 1 => entries.push((list[0], Action::Spawn(c.command.clone()))),
                    Ok(_) => errors.push(format!("{}: {k:?} can't use 1-9", c.id)),
                    Err(e) => errors.push(format!("{}: {k:?}: {e}", c.id)),
                }
            }
        }
        (Bindings { entries }, errors)
    }

    /// Replaces SUPER with other modifiers in every binding. Used when running
    /// nested inside another desktop, which usually keeps SUPER for itself.
    /// The SUPER-tap binding is left alone.
    pub fn remap_logo(mut self, to: Mods) -> Self {
        for (combo, _) in &mut self.entries {
            if combo.mods.logo && !combo.is_logo_tap() {
                combo.mods.logo = false;
                combo.mods = combo.mods.union(to);
            }
        }
        self
    }

    /// Looks up a key press. `sym` should be the unshifted (level 0) symbol,
    /// so SUPER+SHIFT+1 arrives as "1", not "!".
    pub fn lookup(&self, mods: Mods, sym: Keysym) -> Option<&Action> {
        let sym = lower(sym);
        self.entries.iter().find(|(c, _)| !c.is_logo_tap() && c.mods == mods && c.key == sym).map(|(_, a)| a)
    }

    /// The action for tapping SUPER on its own, if any.
    pub fn logo_tap(&self) -> Option<&Action> {
        self.entries.iter().find(|(c, _)| c.is_logo_tap()).map(|(_, a)| a)
    }

    /// Pairs of bindings that share the same combo (the later one is dead).
    pub fn conflicts(&self) -> Vec<(Action, Action)> {
        let mut out = Vec::new();
        for (i, (a, aa)) in self.entries.iter().enumerate() {
            for (b, ba) in &self.entries[i + 1..] {
                if a == b {
                    out.push((aa.clone(), ba.clone()));
                }
            }
        }
        out
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sym(raw: u32) -> Keysym {
        Keysym::from(raw)
    }
    const SUPER: Mods = Mods { logo: true, alt: false, ctrl: false, shift: false };
    const SUPER_SHIFT: Mods = Mods { logo: true, alt: false, ctrl: false, shift: true };
    const ALT: Mods = Mods { logo: false, alt: true, ctrl: false, shift: false };

    #[test]
    fn parses_combos() {
        let c = parse_combo("SUPER+SHIFT+S").unwrap();
        assert_eq!(c.mods, SUPER_SHIFT);
        assert_eq!(c.key, sym(keysyms::KEY_s));

        let c = parse_combo("alt + tab").unwrap();
        assert_eq!(c.mods, ALT);
        assert_eq!(c.key, sym(keysyms::KEY_Tab));

        assert_eq!(parse_combo("PrtSc").unwrap().key, sym(keysyms::KEY_Print));
        assert_eq!(parse_combo("Print").unwrap().key, sym(keysyms::KEY_Print));
        assert_eq!(parse_combo("ctrl+alt+BackSpace").unwrap().key, sym(keysyms::KEY_BackSpace));
        assert_eq!(parse_combo("SUPER+F11").unwrap().key, sym(keysyms::KEY_F11));
        assert_eq!(parse_combo("SUPER+1").unwrap().key, sym(keysyms::KEY_1));
    }

    #[test]
    fn super_alone_is_a_tap() {
        let c = parse_combo("SUPER").unwrap();
        assert!(c.is_logo_tap());
        assert!(!parse_combo("SUPER+S").unwrap().is_logo_tap());
    }

    #[test]
    fn rejects_bad_combos() {
        assert_eq!(parse_combo(""), Err(ParseError::Empty));
        assert_eq!(parse_combo("SUPER+"), Err(ParseError::Empty));
        assert_eq!(parse_combo("HYPER+S"), Err(ParseError::UnknownModifier("HYPER".into())));
        assert_eq!(parse_combo("SHIFT+shift+S"), Err(ParseError::DuplicateModifier("shift".into())));
        assert_eq!(parse_combo("SUPER+NotAKey"), Err(ParseError::UnknownKey("NotAKey".into())));
        // A modifier can't be the key unless it's SUPER on its own.
        assert!(matches!(parse_combo("S+SUPER"), Err(ParseError::UnknownModifier(_))));
    }

    #[test]
    fn errors_are_readable() {
        let e = parse_combo("HYPER+S").unwrap_err().to_string();
        assert!(e.contains("HYPER") && e.contains("SUPER"), "{e}");
    }

    #[test]
    fn lookup_matches_case_insensitively() {
        let b = Bindings::defaults("foot");
        assert_eq!(b.lookup(SUPER_SHIFT, sym(keysyms::KEY_S)), Some(&Action::ScreenshotRegion));
        assert_eq!(b.lookup(SUPER_SHIFT, sym(keysyms::KEY_s)), Some(&Action::ScreenshotRegion));
    }

    #[test]
    fn default_table_matches_the_design() {
        let b = Bindings::defaults("foot");
        let cases = [
            (Mods::NONE, keysyms::KEY_Print, Action::ScreenshotScreen),
            (SUPER_SHIFT, keysyms::KEY_s, Action::ScreenshotRegion),
            (ALT, keysyms::KEY_Tab, Action::AppSwitcher),
            (Mods { logo: true, alt: true, ..Mods::NONE }, keysyms::KEY_Tab, Action::WindowSwitcher),
            (SUPER, keysyms::KEY_s, Action::QuickSettings),
            (SUPER, keysyms::KEY_l, Action::Lock),
            (SUPER, keysyms::KEY_q, Action::CloseWindow),
            (SUPER, keysyms::KEY_Return, Action::Spawn("foot".into())),
            (SUPER, keysyms::KEY_3, Action::Workspace(3)),
            (SUPER_SHIFT, keysyms::KEY_9, Action::MoveToWorkspace(9)),
        ];
        for (m, k, want) in cases {
            assert_eq!(b.lookup(m, sym(k)), Some(&want), "{m:?} {k:#x}");
        }
        assert_eq!(b.logo_tap(), Some(&Action::AppLauncher));
    }

    #[test]
    fn modifiers_must_match_exactly() {
        let b = Bindings::defaults("foot");
        // SUPER+S is quick settings; adding CTRL must not trigger it.
        assert_eq!(b.lookup(Mods { ctrl: true, ..SUPER }, sym(keysyms::KEY_s)), None);
        // Plain "s" while typing is never a shortcut.
        assert_eq!(b.lookup(Mods::NONE, sym(keysyms::KEY_s)), None);
        // The SUPER tap is not a press binding.
        assert_eq!(b.lookup(Mods::NONE, sym(keysyms::KEY_Super_L)), None);
    }

    #[test]
    fn defaults_have_no_conflicts() {
        assert_eq!(Bindings::defaults("foot").conflicts(), vec![]);
    }

    #[test]
    fn nested_remap_moves_super_to_ctrl_alt_without_conflicts() {
        let b = Bindings::defaults("foot").remap_logo(Mods { ctrl: true, alt: true, ..Mods::NONE });
        assert_eq!(b.conflicts(), vec![], "remapped table has clashes");
        let ctrl_alt_shift = Mods { ctrl: true, alt: true, shift: true, logo: false };
        assert_eq!(b.lookup(ctrl_alt_shift, sym(keysyms::KEY_s)), Some(&Action::ScreenshotRegion));
        assert_eq!(b.lookup(SUPER_SHIFT, sym(keysyms::KEY_s)), None);
        // ALT+TAB and PRTSC were never SUPER bindings, so they're unchanged.
        assert_eq!(b.lookup(ALT, sym(keysyms::KEY_Tab)), Some(&Action::AppSwitcher));
        assert_eq!(b.lookup(Mods::NONE, sym(keysyms::KEY_Print)), Some(&Action::ScreenshotScreen));
        assert_eq!(b.logo_tap(), Some(&Action::AppLauncher));
    }

    #[test]
    fn overrides_replace_defaults_and_empty_turns_off() {
        let mut o = std::collections::HashMap::new();
        o.insert("close-window".to_string(), vec!["ALT+F4".to_string(), "SUPER+W".to_string()]);
        o.insert("lock".to_string(), vec![]);
        o.insert("workspace".to_string(), vec!["CTRL+1-9".to_string()]);
        let custom = [CustomShortcut {
            id: "custom-files".into(),
            name: "Files".into(),
            command: "nautilus".into(),
            keys: vec!["SUPER+E".into()],
        }];
        let (b, errors) = Bindings::build("foot", &o, &custom);
        assert_eq!(errors, Vec::<String>::new());
        assert_eq!(b.lookup(ALT, sym(keysyms::KEY_F4)), Some(&Action::CloseWindow));
        assert_eq!(b.lookup(SUPER, sym(keysyms::KEY_w)), Some(&Action::CloseWindow));
        assert_eq!(b.lookup(SUPER, sym(keysyms::KEY_q)), None, "the default is replaced");
        assert_eq!(b.lookup(SUPER, sym(keysyms::KEY_l)), None, "turned off");
        let ctrl = Mods { ctrl: true, ..Mods::NONE };
        assert_eq!(b.lookup(ctrl, sym(keysyms::KEY_7)), Some(&Action::Workspace(7)));
        assert_eq!(b.lookup(SUPER, sym(keysyms::KEY_7)), None);
        assert_eq!(b.lookup(SUPER, sym(keysyms::KEY_e)), Some(&Action::Spawn("nautilus".into())));
    }

    #[test]
    fn bad_overrides_are_reported_not_fatal() {
        let mut o = std::collections::HashMap::new();
        o.insert("close-window".to_string(), vec!["SUPER+1-9".to_string(), "HYPER+Q".to_string()]);
        o.insert("workspace".to_string(), vec!["SUPER+W".to_string()]);
        let (b, errors) = Bindings::build("foot", &o, &[]);
        assert_eq!(errors.len(), 3, "{errors:?}");
        assert_eq!(b.lookup(SUPER, sym(keysyms::KEY_s)), Some(&Action::QuickSettings), "the rest still work");
    }

    #[test]
    fn media_keys_need_no_modifier() {
        let b = Bindings::defaults("foot");
        assert_eq!(b.lookup(Mods::NONE, sym(keysyms::KEY_XF86AudioRaiseVolume)), Some(&Action::Media("volume_up")));
        assert_eq!(
            b.lookup(Mods::NONE, sym(keysyms::KEY_XF86MonBrightnessDown)),
            Some(&Action::Media("brightness_down"))
        );
        assert_eq!(b.lookup(SUPER, sym(keysyms::KEY_space)), Some(&Action::NextLayout));
        assert!(Action::Media("volume_up").is_shell());
    }

    #[test]
    fn every_catalog_entry_has_an_action_and_unique_id() {
        let mut ids = std::collections::HashSet::new();
        for d in CATALOG {
            assert!(ids.insert(d.id), "{} twice", d.id);
            let _ = d.action("foot", 1);
            for c in d.defaults {
                assert!(expand(c).is_ok(), "{} default {c} doesn't parse", d.id);
            }
        }
    }

    #[test]
    fn detects_conflicts() {
        let c = parse_combo("SUPER+F").unwrap();
        let b = Bindings::new(vec![(c, Action::ToggleFullscreen), (c, Action::Spawn("files".into()))]);
        assert_eq!(b.conflicts(), vec![(Action::ToggleFullscreen, Action::Spawn("files".into()))]);
    }

    #[test]
    fn shell_actions_are_classified() {
        assert!(Action::ScreenshotRegion.is_shell());
        assert!(Action::AppLauncher.is_shell());
        assert!(!Action::CloseWindow.is_shell());
        assert!(!Action::AppSwitcher.is_shell(), "the compositor runs the switchers; the shell only draws them");
        assert!(!Action::ScreenshotScreen.is_shell(), "PrtSc is captured by the compositor itself");
        assert!(!Action::Workspace(1).is_shell());
    }
}
