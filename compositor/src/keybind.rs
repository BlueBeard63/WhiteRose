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
                | Action::Notifications
                | Action::ScreenshotRegion
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
        let mut v: Vec<(&str, Action)> = vec![
            ("SUPER", Action::AppLauncher),
            ("ALT+TAB", Action::AppSwitcher),
            ("SUPER+ALT+TAB", Action::WindowSwitcher),
            ("SUPER+S", Action::QuickSettings),
            ("SUPER+L", Action::Lock),
            ("SUPER+N", Action::Notifications),
            ("SUPER+SHIFT+S", Action::ScreenshotRegion),
            ("PRTSC", Action::ScreenshotScreen),
            ("SUPER+Q", Action::CloseWindow),
            ("SUPER+F", Action::ToggleFullscreen),
            ("SUPER+RETURN", Action::Spawn(terminal.to_string())),
            ("CTRL+ALT+BACKSPACE", Action::Quit),
        ];
        let digits = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];
        let ws: Vec<(String, Action)> = digits
            .iter()
            .enumerate()
            .flat_map(|(i, d)| {
                let n = i as u8 + 1;
                [(format!("SUPER+{d}"), Action::Workspace(n)), (format!("SUPER+SHIFT+{d}"), Action::MoveToWorkspace(n))]
            })
            .collect();
        let mut entries: Vec<(Combo, Action)> =
            v.drain(..).map(|(s, a)| (parse_combo(s).expect("default binding parses"), a)).collect();
        entries.extend(ws.into_iter().map(|(s, a)| (parse_combo(&s).expect("default binding parses"), a)));
        Bindings { entries }
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
