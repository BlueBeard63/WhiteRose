//! Keyboard settings and shortcuts, followed live from whiterosed:
//! - "keyboard.layouts" (first is the default; none means the system's,
//!   from /etc/default/keyboard on Ubuntu or the X11 keyboard config on
//!   Fedora), "keyboard.repeatDelay", "keyboard.repeatRate";
//! - "shortcuts.keys" (changes to the built-in shortcuts) and
//!   "shortcuts.custom" (your own), rebuilt into the shortcut table.
//!
//! The Keyboard & Shortcuts page sees the resulting list in the
//! `shortcuts` IPC event, and pauses shortcuts while it records new keys.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use smithay::input::keyboard::XkbConfig;

use crate::{
    ipc,
    keybind::{Bindings, CustomShortcut, Mods, CATALOG},
    state::WhiteRose,
};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Layout {
    pub layout: String,
    #[serde(default)]
    pub variant: String,
}

/// The XKB settings as the system configures them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SystemKeyboard {
    pub layout: String,
    pub variant: String,
    pub options: String,
    pub model: String,
}

/// Debian/Ubuntu's /etc/default/keyboard (shell variables).
pub fn parse_default_keyboard(text: &str) -> SystemKeyboard {
    let mut k = SystemKeyboard::default();
    for line in text.lines() {
        let Some((name, value)) = line.trim().split_once('=') else { continue };
        let value = value.trim().trim_matches('"').trim_matches('\'').to_string();
        match name.trim() {
            "XKBLAYOUT" => k.layout = value,
            "XKBVARIANT" => k.variant = value,
            "XKBOPTIONS" => k.options = value,
            "XKBMODEL" => k.model = value,
            _ => {}
        }
    }
    k
}

/// Fedora's /etc/X11/xorg.conf.d/00-keyboard.conf (written by localectl).
pub fn parse_xorg_keyboard(text: &str) -> SystemKeyboard {
    let mut k = SystemKeyboard::default();
    for line in text.lines() {
        let parts: Vec<&str> = line.split('"').collect();
        if parts.len() < 4 || !parts[0].trim().eq_ignore_ascii_case("Option") {
            continue;
        }
        let value = parts[3].to_string();
        match parts[1] {
            "XkbLayout" => k.layout = value,
            "XkbVariant" => k.variant = value,
            "XkbOptions" => k.options = value,
            "XkbModel" => k.model = value,
            _ => {}
        }
    }
    k
}

pub fn system_keyboard() -> SystemKeyboard {
    if let Ok(t) = std::fs::read_to_string("/etc/default/keyboard") {
        let k = parse_default_keyboard(&t);
        if !k.layout.is_empty() {
            return k;
        }
    }
    if let Ok(t) = std::fs::read_to_string("/etc/X11/xorg.conf.d/00-keyboard.conf") {
        return parse_xorg_keyboard(&t);
    }
    SystemKeyboard::default()
}

/// The layout when nothing else says: UK English.
pub const FALLBACK_LAYOUT: &str = "gb";

/// The XKB layout and variant strings for a list of layouts ("gb,es" and
/// ",nodeadkeys"); the system's when the list is empty, and UK English if
/// the system doesn't say either.
pub fn xkb_names(layouts: &[Layout], system: &SystemKeyboard) -> (String, String) {
    if layouts.is_empty() {
        if system.layout.is_empty() {
            return (FALLBACK_LAYOUT.into(), String::new());
        }
        return (system.layout.clone(), system.variant.clone());
    }
    (
        layouts.iter().map(|l| l.layout.as_str()).collect::<Vec<_>>().join(","),
        layouts.iter().map(|l| l.variant.as_str()).collect::<Vec<_>>().join(","),
    )
}

/// A layout in use, as the settings page and bar show it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutView {
    pub layout: String,
    pub variant: String,
    /// XKB's name for it, e.g. "English (UK)".
    pub name: String,
}

/// One shortcut as the settings page shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShortcutView {
    pub id: String,
    pub name: String,
    pub group: String,
    pub keys: Vec<String>,
    pub defaults: Vec<String>,
    /// Custom shortcuts: what they run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// "digits" or "arrows" for a shortcut that stands for a set of keys
    /// (recorded with one of them); empty for one key.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub range: String,
}

/// Every shortcut with its keys now: built-in (in catalog order), then yours.
pub fn shortcut_views(overrides: &HashMap<String, Vec<String>>, custom: &[CustomShortcut]) -> Vec<ShortcutView> {
    let mut v: Vec<ShortcutView> = CATALOG
        .iter()
        .map(|d| ShortcutView {
            id: d.id.into(),
            name: d.name.into(),
            group: d.group.into(),
            keys: overrides.get(d.id).cloned().unwrap_or_else(|| d.defaults.iter().map(|s| s.to_string()).collect()),
            defaults: d.defaults.iter().map(|s| s.to_string()).collect(),
            command: None,
            range: d.range.name().into(),
        })
        .collect();
    v.extend(custom.iter().map(|c| ShortcutView {
        id: c.id.clone(),
        name: c.name.clone(),
        group: "Custom".into(),
        keys: c.keys.clone(),
        defaults: Vec::new(),
        command: Some(c.command.clone()),
        range: String::new(),
    }));
    v
}

/// The keyboard part of the session's state.
#[derive(Debug, Clone, Default)]
pub struct KeyboardSettings {
    pub layouts: Vec<Layout>,
    pub repeat_delay: i32,
    pub repeat_rate: i32,
    pub overrides: HashMap<String, Vec<String>>,
    pub custom: Vec<CustomShortcut>,
    /// The terminal SUPER+RETURN opens.
    pub terminal: String,
    /// Nested with --mod ctrl-alt: SUPER in every shortcut becomes these.
    pub logo_remap: Option<Mods>,
    /// The layouts in use, with XKB's names, and the active one.
    pub view: Vec<LayoutView>,
    pub active: usize,
}

impl WhiteRose {
    /// Takes one setting from whiterosed. Returns (keyboard changed,
    /// shortcuts changed); unrelated keys change nothing.
    pub fn keyboard_setting(&mut self, key: &str, value: &Value) -> (bool, bool) {
        let k = &mut self.keyboard;
        fn set<T: PartialEq + for<'a> Deserialize<'a>>(slot: &mut T, key: &str, value: &Value) -> bool {
            match serde_json::from_value::<T>(value.clone()) {
                Ok(v) if v != *slot => {
                    *slot = v;
                    true
                }
                Ok(_) => false,
                Err(e) => {
                    tracing::warn!(key, %e, "ignoring unreadable setting");
                    false
                }
            }
        }
        match key {
            "keyboard.layouts" => (set(&mut k.layouts, key, value), false),
            "keyboard.repeatDelay" => (set(&mut k.repeat_delay, key, value), false),
            "keyboard.repeatRate" => (set(&mut k.repeat_rate, key, value), false),
            "shortcuts.keys" => (false, set(&mut k.overrides, key, value)),
            "shortcuts.custom" => (false, set(&mut k.custom, key, value)),
            _ => (false, false),
        }
    }

    /// Applies layouts and key repeat to the seat's keyboard.
    pub fn apply_keyboard(&mut self) {
        let system = system_keyboard();
        let (layout, variant) = xkb_names(&self.keyboard.layouts, &system);
        let config = XkbConfig {
            layout: &layout,
            variant: &variant,
            options: (!system.options.is_empty()).then(|| system.options.clone()),
            model: &system.model,
            ..Default::default()
        };
        let keyboard = self.seat.get_keyboard().expect("seat has a keyboard");
        match keyboard.set_xkb_config(self, config) {
            Ok(()) => tracing::info!(layout, variant, "keyboard layout"),
            Err(err) => tracing::warn!(layout, variant, ?err, "keyboard layout refused"),
        }
        let (rate, delay) = (self.keyboard.repeat_rate, self.keyboard.repeat_delay);
        if rate > 0 && delay > 0 {
            keyboard.change_repeat_info(rate, delay);
        }
        self.keyboard_sync();
    }

    /// Re-reads the layout names and the active one from XKB and tells the
    /// shell (after a layout change or switch).
    fn keyboard_sync(&mut self) {
        let system = system_keyboard();
        let (layout, variant) = xkb_names(&self.keyboard.layouts, &system);
        let keyboard = self.seat.get_keyboard().expect("seat has a keyboard");
        let (names, active) = keyboard.with_xkb_state(self, |ctx| {
            let xkb = ctx.xkb().lock().unwrap();
            let names: Vec<String> = xkb.layouts().map(|l| xkb.layout_name(l).to_string()).collect();
            (names, xkb.active_layout().0 as usize)
        });
        let variants: Vec<&str> = variant.split(',').collect();
        self.keyboard.view = layout
            .split(',')
            .enumerate()
            .filter(|(_, l)| !l.is_empty())
            .map(|(i, l)| LayoutView {
                layout: l.to_string(),
                variant: variants.get(i).copied().unwrap_or("").to_string(),
                name: names.get(i).cloned().unwrap_or_else(|| l.to_string()),
            })
            .collect();
        self.keyboard.active = active;
        let e = self.keyboard_event();
        if let Some(ipc) = self.ipc.as_mut() {
            ipc.update(e);
        }
    }

    pub fn keyboard_event(&self) -> ipc::Event {
        ipc::Event::Keyboard { layouts: self.keyboard.view.clone(), active: self.keyboard.active }
    }

    /// Rebuilds the shortcut table and tells the settings page.
    pub fn apply_shortcuts(&mut self) {
        let k = &self.keyboard;
        let (mut bindings, errors) = Bindings::build(&k.terminal, &k.overrides, &k.custom);
        for e in errors {
            tracing::warn!("shortcut skipped: {e}");
        }
        if let Some(to) = k.logo_remap {
            bindings = bindings.remap_logo(to);
        }
        self.bindings = bindings;
        self.shortcuts_sync();
    }

    pub fn shortcuts_event(&self) -> ipc::Event {
        ipc::Event::Shortcuts { shortcuts: shortcut_views(&self.keyboard.overrides, &self.keyboard.custom) }
    }

    pub fn shortcuts_sync(&mut self) {
        let e = self.shortcuts_event();
        if let Some(ipc) = self.ipc.as_mut() {
            ipc.update(e);
        }
    }

    /// SUPER+SPACE: the next layout, if there's more than one.
    pub fn next_layout(&mut self) {
        let keyboard = self.seat.get_keyboard().expect("seat has a keyboard");
        keyboard.with_xkb_state(self, |mut ctx| ctx.cycle_next_layout());
        self.keyboard_sync();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_ubuntu_keyboard_config() {
        let k = parse_default_keyboard(
            "# KEYBOARD CONFIGURATION FILE\nXKBMODEL=\"pc105\"\nXKBLAYOUT=\"gb\"\nXKBVARIANT=\"\"\nXKBOPTIONS=\"caps:escape\"\n\nBACKSPACE=\"guess\"\n",
        );
        assert_eq!(
            k,
            SystemKeyboard {
                layout: "gb".into(),
                variant: "".into(),
                options: "caps:escape".into(),
                model: "pc105".into()
            }
        );
    }

    #[test]
    fn reads_fedora_keyboard_config() {
        let k = parse_xorg_keyboard(
            "Section \"InputClass\"\n        Identifier \"system-keyboard\"\n        MatchIsKeyboard \"on\"\n        Option \"XkbLayout\" \"us,de\"\n        Option \"XkbVariant\" \",nodeadkeys\"\nEndSection\n",
        );
        assert_eq!((k.layout.as_str(), k.variant.as_str()), ("us,de", ",nodeadkeys"));
    }

    #[test]
    fn layouts_join_and_fall_back_to_the_system() {
        let sys = SystemKeyboard { layout: "gb".into(), ..Default::default() };
        assert_eq!(xkb_names(&[], &sys), ("gb".into(), "".into()));
        let mine = [
            Layout { layout: "gb".into(), variant: "".into() },
            Layout { layout: "es".into(), variant: "nodeadkeys".into() },
        ];
        assert_eq!(xkb_names(&mine, &sys), ("gb,es".into(), ",nodeadkeys".into()));
        assert_eq!(xkb_names(&[], &SystemKeyboard::default()), ("gb".into(), "".into()), "UK when nothing says");
    }

    #[test]
    fn views_show_overrides_defaults_and_custom() {
        let mut o = HashMap::new();
        o.insert("lock".to_string(), vec![]);
        let custom = [CustomShortcut {
            id: "custom-files".into(),
            name: "Open Files".into(),
            command: "nautilus".into(),
            keys: vec!["SUPER+E".into()],
        }];
        let v = shortcut_views(&o, &custom);
        let lock = v.iter().find(|s| s.id == "lock").unwrap();
        assert_eq!((lock.keys.clone(), lock.defaults.clone()), (vec![], vec!["SUPER+L".to_string()]));
        let close = v.iter().find(|s| s.id == "close-window").unwrap();
        assert_eq!(close.keys, close.defaults);
        let last = v.last().unwrap();
        assert_eq!((last.group.as_str(), last.command.as_deref()), ("Custom", Some("nautilus")));
    }
}
