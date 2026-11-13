//! Keyboard shortcuts: parsing ("SUPER+SHIFT+S"), the default table, and
//! lookup. Pure logic, no compositor state, so it is fully unit-tested.

use crate::layout::Dir;
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
/// The compositor's exit code asking whiterose-launch to start the session again.
pub const RESTART_CODE: i32 = 42;

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
    /// The island's clipboard history (SUPER+V).
    Clipboard,
    /// The note in the island (SUPER+SHIFT+N; the Note module).
    Note,
    /// A snapshot for a bug report, the moment something goes wrong
    /// (SUPER+SHIFT+B; whiterosed takes it, the island offers to report).
    ReportSnapshot,
    /// Today's agenda in the island (SUPER+A; the Agenda module).
    Agenda,
    /// What's playing, in the island (SUPER+SHIFT+M; the Music module).
    Music,
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
    /// Move the focused window to the workspace before (left, up) or after
    /// (right, down) this one, and go there with it.
    MoveToWorkspaceStep(Dir),
    /// Move the focused window to the monitor on that side.
    MoveToOutput(Dir),
    Spawn(String),
    /// An app from the launcher or dock (its desktop entry): its app id, its
    /// command line already split, and where to start it.
    Launch {
        app_id: String,
        argv: Vec<String>,
        cwd: Option<String>,
    },
    /// A scratchpad's key (its id): start, show, focus or hide it.
    Scratchpad(String),
    /// Focus a window by its IPC id (switching workspace if needed).
    FocusWindow(u64),
    /// Focus an app's most recent window, or cycle if it already has focus.
    FocusApp(String),
    /// Delete a screenshot this compositor saved (requested by the shell).
    DeleteScreenshot(String),
    /// Night light on/off and its warmth (from the shell's quick settings).
    NightLight(crate::night_light::NightLight),
    /// Clipboard and screenshot-tray requests from the shell.
    Shell(ShellRequest),
    /// Ctrl+Alt+Fn: switch to console n (real hardware only).
    SwitchVt(i32),
    /// The screenshot tool's capture (region, pointer, save or copy).
    Capture(crate::screenshot::Shot),
    /// The next keyboard layout (SUPER+SPACE).
    NextLayout,
    /// The next window layout (floating, dwindle, master) for this
    /// workspace, saved as its setting: on the output named (the bar's
    /// button), else where you're working (SUPER+T).
    NextWindowLayout(Option<String>),
    /// Take the focused window out of the tiles, or put it back.
    ToggleFloating,
    /// Focus the window beside the focused one.
    FocusDirection(Dir),
    /// Swap the focused tile with the one beside it (a floating window
    /// moves a step that way).
    MoveDirection(Dir),
    /// Right and down grow the focused window, left and up shrink it (the
    /// arrows in resize mode).
    ResizeDirection(Dir),
    /// Resize mode (SUPER+R): the arrow keys resize the focused window
    /// until Escape, Enter or any other key.
    ResizeMode,
    /// Dwindle: flip the split the focused tile is in.
    ToggleSplit,
    /// Master: make the focused tile the master.
    MakeMaster,
    /// Join the last used tile's group, or leave the group.
    ToggleGroup,
    /// The snap layout picker: for the focused window (SUPER+Z), or the
    /// window under the pointer (true: a WhiteRose app's maximize button).
    SnapPicker(bool),
    /// Show the next (or previous) tab of the focused group.
    NextTab,
    PreviousTab,
    /// Volume, brightness and media keys, done by the shell; the name is
    /// its wire name (e.g. "volume_up").
    Media(&'static str),
    Quit,
    /// End the session asking the launcher to start it again (an update,
    /// "Try again"): exit code `RESTART_CODE`.
    Restart,
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
                | Action::Clipboard
                | Action::Note
                | Action::ReportSnapshot
                | Action::Agenda
                | Action::Music
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

/// The arrow keys, in the order an "ARROWS" range lists them.
pub const ARROWS: [(&str, Dir); 4] = [("Left", Dir::Left), ("Right", Dir::Right), ("Up", Dir::Up), ("Down", Dir::Down)];

/// "SUPER+1-9" is nine combos (1 to 9), "SUPER+ARROWS" four (left, right,
/// up, down); anything else is one.
pub fn expand(s: &str) -> Result<Vec<Combo>, ParseError> {
    match s.rsplit_once('+') {
        Some((mods, "1-9")) => (1..=9).map(|d| parse_combo(&format!("{mods}+{d}"))).collect(),
        Some((mods, key)) if key.eq_ignore_ascii_case("ARROWS") => {
            ARROWS.iter().map(|(k, _)| parse_combo(&format!("{mods}+{k}"))).collect()
        }
        None if s == "1-9" || s.eq_ignore_ascii_case("ARROWS") => Err(ParseError::UnknownKey(s.into())),
        _ => parse_combo(s).map(|c| vec![c]),
    }
}

/// A shortcut that stands for a set of keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Range {
    /// One key.
    No,
    /// "MODS+1-9": workspace n for key n.
    Digits,
    /// "MODS+ARROWS": that direction for each arrow key.
    Arrows,
}

impl Range {
    /// How the settings page knows (empty for one key).
    pub fn name(self) -> &'static str {
        match self {
            Range::No => "",
            Range::Digits => "digits",
            Range::Arrows => "arrows",
        }
    }

    fn len(self) -> usize {
        match self {
            Range::No => 1,
            Range::Digits => 9,
            Range::Arrows => 4,
        }
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
/// and grouped under on the Keyboard & shortcuts page, and its default keys.
#[derive(Debug, Clone, Copy)]
pub struct Def {
    pub id: &'static str,
    pub name: &'static str,
    pub group: &'static str,
    pub defaults: &'static [&'static str],
    /// Bound to a set of keys ("MODS+1-9", "MODS+ARROWS").
    pub range: Range,
}

impl Def {
    fn action(&self, terminal: &str, n: u8) -> Action {
        match self.id {
            "launcher" => Action::AppLauncher,
            "app-switcher" => Action::AppSwitcher,
            "window-switcher" => Action::WindowSwitcher,
            "quick-settings" => Action::QuickSettings,
            "notifications" => Action::Notifications,
            "clipboard" => Action::Clipboard,
            "note" => Action::Note,
            "report" => Action::ReportSnapshot,
            "agenda" => Action::Agenda,
            "music" => Action::Music,
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
            "move-to-workspace-step" => Action::MoveToWorkspaceStep(ARROWS[n as usize].1),
            "move-to-monitor" => Action::MoveToOutput(ARROWS[n as usize].1),
            "volume-up" => Action::Media("volume_up"),
            "volume-down" => Action::Media("volume_down"),
            "volume-mute" => Action::Media("volume_mute"),
            "mic-mute" => Action::Media("mic_mute"),
            "brightness-up" => Action::Media("brightness_up"),
            "brightness-down" => Action::Media("brightness_down"),
            "play-pause" => Action::Media("media_play_pause"),
            "next-track" => Action::Media("media_next"),
            "previous-track" => Action::Media("media_previous"),
            "window-layout" => Action::NextWindowLayout(None),
            "toggle-floating" => Action::ToggleFloating,
            "focus-direction" => Action::FocusDirection(ARROWS[n as usize].1),
            "move-direction" => Action::MoveDirection(ARROWS[n as usize].1),
            "resize-mode" => Action::ResizeMode,
            "toggle-split" => Action::ToggleSplit,
            "make-master" => Action::MakeMaster,
            "toggle-group" => Action::ToggleGroup,
            "snap-picker" => Action::SnapPicker(false),
            "next-tab" => Action::NextTab,
            "previous-tab" => Action::PreviousTab,
            other => unreachable!("catalog id {other} has no action"),
        }
    }
}

const fn def(id: &'static str, name: &'static str, group: &'static str, defaults: &'static [&'static str]) -> Def {
    Def { id, name, group, defaults, range: Range::No }
}

/// Every built-in shortcut, in the order the settings page lists them.
pub const CATALOG: &[Def] = &[
    def("launcher", "App launcher", "Shell", &["SUPER"]),
    def("app-switcher", "Application switcher", "Windows", &["ALT+TAB"]),
    def("window-switcher", "Window switcher", "Windows", &["SUPER+TAB"]),
    def("quick-settings", "Quick settings", "Shell", &["SUPER+S"]),
    def("notifications", "Notifications", "Shell", &["SUPER+N"]),
    def("clipboard", "Clipboard history", "Shell", &["SUPER+V"]),
    def("note", "Note", "Shell", &["SUPER+SHIFT+N"]),
    def("agenda", "Today's agenda", "Shell", &["SUPER+A"]),
    def("music", "Now playing", "Shell", &["SUPER+SHIFT+M"]),
    def("report", "Snapshot for a bug report", "Shell", &["SUPER+SHIFT+B"]),
    def("fullscreen", "Toggle fullscreen", "Windows", &["SUPER+F"]),
    def("close-window", "Close window", "Windows", &["SUPER+Q"]),
    def("window-layout", "Next window layout", "Windows", &["SUPER+T"]),
    def("toggle-floating", "Float or tile window", "Windows", &["SUPER+SHIFT+SPACE"]),
    Def { range: Range::Arrows, ..def("focus-direction", "Focus window ←→↑↓", "Windows", &["SUPER+ARROWS"]) },
    Def {
        range: Range::Arrows,
        ..def("move-direction", "Move or snap window ←→↑↓", "Windows", &["SUPER+SHIFT+ARROWS"])
    },
    Def {
        range: Range::Arrows,
        ..def("move-to-monitor", "Move window to the monitor ←→↑↓", "Windows", &["SUPER+CTRL+ARROWS"])
    },
    def("resize-mode", "Resize window (then arrows; Esc ends)", "Windows", &["SUPER+R"]),
    def("toggle-split", "Flip split (dwindle)", "Windows", &["SUPER+J"]),
    def("make-master", "Make master (master)", "Windows", &["SUPER+M"]),
    def("toggle-group", "Group or ungroup window", "Windows", &["SUPER+G"]),
    def("snap-picker", "Snap layouts", "Windows", &["SUPER+Z"]),
    def("next-tab", "Next tab", "Windows", &["SUPER+PERIOD"]),
    def("previous-tab", "Previous tab", "Windows", &["SUPER+COMMA"]),
    Def { range: Range::Digits, ..def("workspace", "Switch to workspace 1–9", "Workspaces", &["SUPER+1-9"]) },
    Def {
        range: Range::Digits,
        ..def("move-to-workspace", "Move window to workspace 1–9", "Workspaces", &["SUPER+SHIFT+1-9"])
    },
    Def {
        range: Range::Arrows,
        ..def(
            "move-to-workspace-step",
            "Move window to the previous / next workspace ←→↑↓",
            "Workspaces",
            &["SUPER+ALT+ARROWS"],
        )
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

    /// WhiteRose's default shortcuts (see the Keyboard & shortcuts design).
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
                    Ok(list) if def.range != Range::No => {
                        let wanted = if def.range == Range::Digits { "1-9" } else { "ARROWS" };
                        if list.len() != def.range.len() || !c.to_ascii_uppercase().ends_with(wanted) {
                            errors.push(format!("{}: {c:?} needs the {wanted} key", def.id));
                            continue;
                        }
                        for (n, combo) in list.into_iter().enumerate() {
                            // Workspaces count from 1; directions index ARROWS.
                            let n = if def.range == Range::Digits { n + 1 } else { n };
                            entries.push((combo, def.action(terminal, n as u8)));
                        }
                    }
                    Ok(list) if list.len() == 1 => entries.push((list[0], def.action(terminal, 0))),
                    Ok(_) => errors.push(format!("{}: {c:?} can't use a range of keys", def.id)),
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

    /// Adds the scratchpads' keys, so `conflicts()` sees them against the
    /// rest. Keys that don't parse are skipped and reported.
    pub fn add_scratchpads(&mut self, pads: &[crate::scratchpad::Pad]) -> Vec<String> {
        let mut errors = Vec::new();
        for p in pads {
            for k in &p.keys {
                match expand(k) {
                    Ok(list) if list.len() == 1 => self.entries.push((list[0], Action::Scratchpad(p.id.clone()))),
                    Ok(_) => errors.push(format!("{}: {k:?} can't use a range of keys", p.id)),
                    Err(e) => errors.push(format!("{}: {k:?}: {e}", p.id)),
                }
            }
        }
        errors
    }

    /// Replaces SUPER with other modifiers in every binding. Used when running
    /// nested inside another desktop, which usually keeps SUPER for itself.
    /// The SUPER-tap binding is left alone.
    pub fn remap_logo(mut self, to: Mods) -> Self {
        // One that would land on another's keys (SUPER+ALT+arrow and
        // SUPER+arrow both becoming CTRL+ALT+arrow) is left as it was:
        // out of reach in that mode, rather than taking another's keys.
        let remapped = |c: &Combo| {
            let mut c = *c;
            if c.mods.logo && !c.is_logo_tap() {
                c.mods.logo = false;
                c.mods = c.mods.union(to);
            }
            c
        };
        let new: Vec<Combo> = self.entries.iter().map(|(c, _)| remapped(c)).collect();
        for i in 0..self.entries.len() {
            let clash =
                new.iter().enumerate().any(|(j, n)| j < i && *n == new[i] && self.entries[j].0 != self.entries[i].0);
            if !clash {
                self.entries[i].0 = new[i];
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
    const SUPER_ALT: Mods = Mods { logo: true, alt: true, ctrl: false, shift: false };
    const SUPER_CTRL: Mods = Mods { logo: true, alt: false, ctrl: true, shift: false };
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
            (SUPER, keysyms::KEY_v, Action::Clipboard),
            (SUPER_SHIFT, keysyms::KEY_n, Action::Note),
            (SUPER, keysyms::KEY_a, Action::Agenda),
            (SUPER_SHIFT, keysyms::KEY_m, Action::Music),
            (SUPER_SHIFT, keysyms::KEY_b, Action::ReportSnapshot),
            (SUPER_ALT, keysyms::KEY_Left, Action::MoveToWorkspaceStep(Dir::Left)),
            (SUPER_ALT, keysyms::KEY_Down, Action::MoveToWorkspaceStep(Dir::Down)),
            (SUPER_CTRL, keysyms::KEY_Right, Action::MoveToOutput(Dir::Right)),
            (ALT, keysyms::KEY_Tab, Action::AppSwitcher),
            (SUPER, keysyms::KEY_Tab, Action::WindowSwitcher),
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
        // SUPER+ALT+arrow would land on SUPER+arrow's keys: left as it was.
        let ctrl_alt = Mods { ctrl: true, alt: true, ..Mods::NONE };
        assert_eq!(b.lookup(ctrl_alt, sym(keysyms::KEY_Left)), Some(&Action::FocusDirection(Dir::Left)));
        let super_alt = Mods { logo: true, alt: true, ..Mods::NONE };
        assert_eq!(b.lookup(super_alt, sym(keysyms::KEY_Left)), Some(&Action::MoveToWorkspaceStep(Dir::Left)));
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
    fn scratchpad_keys_bind_and_clash() {
        let pads = crate::scratchpad::parse(&serde_json::json!([
            {"id": "term", "command": "foot", "keys": ["SUPER+GRAVE"]},
            {"id": "notes", "command": "notes", "keys": ["SUPER+F", "SUPER+NOPE", "SUPER+1-9"]}
        ]));
        let mut b = Bindings::defaults("foot");
        let errors = b.add_scratchpads(&pads);
        assert_eq!(errors.len(), 2, "{errors:?}");
        let grave = parse_combo("SUPER+GRAVE").unwrap();
        assert_eq!(b.lookup(grave.mods, grave.key), Some(&Action::Scratchpad("term".into())));
        assert_eq!(b.conflicts(), vec![(Action::ToggleFullscreen, Action::Scratchpad("notes".into()))]);
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

    #[test]
    fn arrows_stand_for_four_keys() {
        let combos = expand("SUPER+SHIFT+ARROWS").unwrap();
        let keys: Vec<u32> = combos.iter().map(|c| c.key.raw()).collect();
        assert_eq!(keys, vec![keysyms::KEY_Left, keysyms::KEY_Right, keysyms::KEY_Up, keysyms::KEY_Down]);
        assert!(combos.iter().all(|c| c.mods == SUPER_SHIFT));
        assert_eq!(expand("super+arrows").unwrap().len(), 4, "any case");
        assert!(expand("ARROWS").is_err(), "needs a modifier");
    }

    #[test]
    fn the_default_tiling_keys() {
        let b = Bindings::defaults("true");
        let key = |m, k| b.lookup(m, sym(k)).cloned();
        assert_eq!(key(SUPER, keysyms::KEY_t), Some(Action::NextWindowLayout(None)));
        assert_eq!(key(SUPER_SHIFT, keysyms::KEY_space), Some(Action::ToggleFloating));
        assert_eq!(key(SUPER, keysyms::KEY_Left), Some(Action::FocusDirection(Dir::Left)));
        assert_eq!(key(SUPER_SHIFT, keysyms::KEY_Down), Some(Action::MoveDirection(Dir::Down)));
        assert_eq!(key(SUPER, keysyms::KEY_r), Some(Action::ResizeMode));
        assert_eq!(key(SUPER, keysyms::KEY_period), Some(Action::NextTab));
        assert_eq!(key(SUPER, keysyms::KEY_comma), Some(Action::PreviousTab));
        assert_eq!(key(SUPER, keysyms::KEY_g), Some(Action::ToggleGroup));
    }

    #[test]
    fn a_range_shortcut_needs_its_range_key() {
        let mut o = std::collections::HashMap::new();
        o.insert("focus-direction".to_string(), vec!["SUPER+1-9".to_string(), "SUPER+K".to_string()]);
        o.insert("workspace".to_string(), vec!["SUPER+ARROWS".to_string()]);
        let (b, errors) = Bindings::build("true", &o, &[]);
        assert_eq!(errors.len(), 3, "{errors:?}");
        assert_eq!(b.lookup(SUPER, sym(keysyms::KEY_k)), None);
        assert_eq!(b.lookup(SUPER, sym(keysyms::KEY_Left)), None);
    }
}
