//! Mice and touchpads (Settings › Mouse & Touchpad): scroll direction,
//! speed, acceleration and tap to click, each for mice and touchpads
//! separately, applied through libinput to every device when it's plugged
//! in and again when a setting changes. Only the real-hardware backend has
//! libinput devices: in a window on another desktop, that desktop's own
//! settings apply.

use serde::Deserialize;
use serde_json::Value;
use smithay::reexports::input::{AccelProfile, Device, DeviceCapability};

use crate::{keybind::Mods, state::WhiteRose};

/// One kind of device's settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeviceSettings {
    /// Content follows the fingers (scrolling down moves the page up).
    pub natural_scroll: bool,
    /// libinput's speed, -1 (slowest) to 1 (fastest).
    pub speed: f64,
    /// Faster movements carry the pointer further (off: flat).
    pub acceleration: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerSettings {
    pub mouse: DeviceSettings,
    pub touchpad: DeviceSettings,
    pub tap_to_click: bool,
    /// The modifiers to hold for a faster pointer (None: off), and how much
    /// faster.
    pub faster_with: Option<Mods>,
    pub faster_by: f64,
}

impl Default for PointerSettings {
    fn default() -> Self {
        // The usual: a wheel scrolls the traditional way, a touchpad the
        // natural way (as on phones).
        PointerSettings {
            mouse: DeviceSettings { natural_scroll: false, speed: 0.0, acceleration: true },
            touchpad: DeviceSettings { natural_scroll: true, speed: 0.0, acceleration: true },
            tap_to_click: true,
            faster_with: Some(Mods { ctrl: true, shift: true, ..Mods::NONE }),
            faster_by: 3.0,
        }
    }
}

impl PointerSettings {
    /// Takes a setting; true if it changed something.
    pub fn take(&mut self, key: &str, value: &Value) -> bool {
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
        let changed = match key {
            "mouse.naturalScroll" => set(&mut self.mouse.natural_scroll, key, value),
            "mouse.speed" => set(&mut self.mouse.speed, key, value),
            "mouse.acceleration" => set(&mut self.mouse.acceleration, key, value),
            "touchpad.naturalScroll" => set(&mut self.touchpad.natural_scroll, key, value),
            "touchpad.speed" => set(&mut self.touchpad.speed, key, value),
            "touchpad.acceleration" => set(&mut self.touchpad.acceleration, key, value),
            "touchpad.tapToClick" => set(&mut self.tap_to_click, key, value),
            // (Pointer motion only: nothing for libinput to apply.)
            "mouse.fasterWith" => {
                self.faster_with = match value.as_str() {
                    Some("CTRL+SHIFT") => Some(Mods { ctrl: true, shift: true, ..Mods::NONE }),
                    Some("CTRL+ALT") => Some(Mods { ctrl: true, alt: true, ..Mods::NONE }),
                    Some("SUPER+SHIFT") => Some(Mods { logo: true, shift: true, ..Mods::NONE }),
                    _ => None,
                };
                return false;
            }
            "mouse.fasterBy" => {
                set(&mut self.faster_by, key, value);
                self.faster_by = self.faster_by.clamp(1.0, 8.0);
                return false;
            }
            _ => false,
        };
        self.mouse.speed = self.mouse.speed.clamp(-1.0, 1.0);
        self.touchpad.speed = self.touchpad.speed.clamp(-1.0, 1.0);
        changed
    }

    /// Sets `device` up: a touchpad (it can tap) or a mouse.
    pub fn apply(&self, device: &mut Device) {
        if !device.has_capability(DeviceCapability::Pointer) {
            return;
        }
        let touchpad = device.config_tap_finger_count() > 0;
        let s = if touchpad { self.touchpad } else { self.mouse };
        if touchpad {
            let _ = device.config_tap_set_enabled(self.tap_to_click);
        }
        if device.config_scroll_has_natural_scroll() {
            let _ = device.config_scroll_set_natural_scroll_enabled(s.natural_scroll);
        }
        if device.config_accel_is_available() {
            let _ = device.config_accel_set_speed(s.speed);
            let profile = if s.acceleration { AccelProfile::Adaptive } else { AccelProfile::Flat };
            if device.config_accel_profiles().contains(&profile) {
                let _ = device.config_accel_set_profile(profile);
            }
        }
    }
}

impl WhiteRose {
    /// A device was plugged in (or found at start).
    pub fn add_input_device(&mut self, mut device: Device) {
        self.pointer.apply(&mut device);
        self.input_devices.push(device);
    }

    pub fn remove_input_device(&mut self, device: &Device) {
        self.input_devices.retain(|d| d != device);
    }

    /// Takes a mouse or touchpad setting, applying it to every device.
    pub fn pointer_setting(&mut self, key: &str, value: &Value) {
        if key == "mouse.shakeToFind" {
            self.shake.enabled = value.as_bool().unwrap_or(true);
            return;
        }
        if self.pointer.take(key, value) {
            let settings = self.pointer;
            for d in &mut self.input_devices {
                settings.apply(d);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn takes_settings_and_keeps_speed_in_range() {
        let mut p = PointerSettings::default();
        assert!(p.take("mouse.naturalScroll", &json!(true)));
        assert!(!p.take("mouse.naturalScroll", &json!(true)), "unchanged");
        assert!(p.mouse.natural_scroll && p.touchpad.natural_scroll);
        assert!(p.take("touchpad.speed", &json!(0.4)));
        assert_eq!(p.touchpad.speed, 0.4);
        assert!(p.take("mouse.speed", &json!(7)));
        assert_eq!(p.mouse.speed, 1.0, "clamped");
        assert!(p.take("touchpad.tapToClick", &json!(false)));
        assert!(!p.tap_to_click);
        assert!(!p.take("mouse.acceleration", &json!("fast")), "unreadable: ignored");
        assert!(p.mouse.acceleration);
        assert!(!p.take("keyboard.repeatRate", &json!(30)));
        // Not libinput's, so nothing to re-apply; kept in range.
        assert!(!p.take("mouse.fasterBy", &json!(4)));
        assert_eq!(p.faster_by, 4.0);
        p.take("mouse.fasterBy", &json!(50));
        assert_eq!(p.faster_by, 8.0);
        assert_eq!(p.faster_with, Some(Mods { ctrl: true, shift: true, ..Mods::NONE }), "Ctrl+Shift to start with");
        p.take("mouse.fasterWith", &json!("SUPER+SHIFT"));
        assert_eq!(p.faster_with, Some(Mods { logo: true, shift: true, ..Mods::NONE }));
        p.take("mouse.fasterWith", &json!(""));
        assert_eq!(p.faster_with, None, "off");
    }
}
