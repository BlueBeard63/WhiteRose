//! Input handling: shortcuts first, then forwarding to clients.
//! Pointer handling follows Smithay's smallvil example (MIT).

use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, InputBackend, InputEvent, KeyState,
        KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent, PointerMotionEvent,
    },
    desktop::Window,
    input::{
        keyboard::FilterResult,
        pointer::{AxisFrame, ButtonEvent, GrabStartData, MotionEvent},
    },
    utils::{Logical, Point, Serial, SERIAL_COUNTER},
};

use crate::{
    grabs::tile_grab::nearest_corner,
    keybind::{is_logo_key, Action, Mods},
    state::WhiteRose,
};

impl WhiteRose {
    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        match event {
            InputEvent::Keyboard { event, .. } => {
                self.on_key(event.key_code(), event.state(), Event::time_msec(&event))
            }
            InputEvent::PointerMotionAbsolute { event, .. } => {
                let Some(output) = self.space.outputs().next() else { return };
                let Some(output_geo) = self.space.output_geometry(output) else { return };
                let pos = event.position_transformed(output_geo.size) + output_geo.loc.to_f64();
                self.on_pointer_motion(pos, event.time_msec());
            }
            InputEvent::PointerMotion { event, .. } => {
                let pointer = self.seat.get_pointer().expect("seat has a pointer");
                let pos = self.clamp_to_outputs(pointer.current_location(), pointer.current_location() + event.delta());
                self.on_pointer_motion(pos, event.time_msec());
            }
            InputEvent::PointerButton { event, .. } => {
                self.on_pointer_button(event.button_code(), event.state(), event.time_msec())
            }
            InputEvent::PointerAxis { event, .. } => {
                self.activity();
                let source = event.source();
                let amount =
                    |axis| event.amount(axis).unwrap_or_else(|| event.amount_v120(axis).unwrap_or(0.0) * 15.0 / 120.);
                let mut frame = AxisFrame::new(event.time_msec()).source(source);
                for axis in [Axis::Horizontal, Axis::Vertical] {
                    let v = amount(axis);
                    if v != 0.0 {
                        frame = frame.value(axis, v);
                        if let Some(discrete) = event.amount_v120(axis) {
                            frame = frame.v120(axis, discrete as i32);
                        }
                    }
                    if source == AxisSource::Finger && event.amount(axis) == Some(0.0) {
                        frame = frame.stop(axis);
                    }
                }
                let pointer = self.seat.get_pointer().expect("seat has a pointer");
                pointer.axis(self, frame);
                pointer.frame(self);
            }
            _ => {}
        }
    }

    /// The pointer moved to `pos` (global coordinates). Also driven by tests.
    pub fn on_pointer_motion(&mut self, pos: Point<f64, Logical>, time: u32) {
        self.activity();
        let serial = SERIAL_COUNTER.next_serial();
        let pointer = self.seat.get_pointer().expect("seat has a pointer");
        let under = self.surface_under(pos);
        pointer.motion(self, under, &MotionEvent { location: pos, serial, time });
        pointer.frame(self);
        // The drag icon follows the pointer, and so does a pointer we draw.
        if self.dnd_icon.is_some() || self.draws_cursor {
            self.schedule_redraw();
        }
    }

    /// Where a relative move from `from` to `to` ends up: anywhere on a
    /// screen, else stopped at the edge of the screen it started on (so the
    /// pointer slides along edges and can't get lost in gaps between them).
    pub fn clamp_to_outputs(&self, from: Point<f64, Logical>, to: Point<f64, Logical>) -> Point<f64, Logical> {
        let geos: Vec<_> = self.space.outputs().filter_map(|o| self.space.output_geometry(o)).collect();
        if geos.iter().any(|g| g.to_f64().contains(to)) {
            return to;
        }
        let Some(g) = geos.iter().find(|g| g.to_f64().contains(from)).or(geos.first()) else { return to };
        let max = (g.loc + g.size).to_f64();
        Point::from((to.x.clamp(g.loc.x as f64, max.x - 1.0), to.y.clamp(g.loc.y as f64, max.y - 1.0)))
    }

    /// A pointer button changed (`button` is a Linux input code, e.g. 0x110
    /// for the left button). Also driven by tests.
    pub fn on_pointer_button(&mut self, button: u32, state: ButtonState, time: u32) {
        self.activity();
        self.logo_tap_pending = false;
        let pointer = self.seat.get_pointer().expect("seat has a pointer");
        let serial = SERIAL_COUNTER.next_serial();
        let button_state = state;

        if button_state == ButtonState::Pressed && self.is_locked() {
            self.focus_lock_surface();
        } else if button_state == ButtonState::Pressed && !pointer.is_grabbed() {
            let at = pointer.current_location();
            if let Some(layer) = self.top_layer_at(at) {
                // Clicks on panels never change window focus. Panels that
                // take typing (e.g. a search field) get the keyboard.
                if layer.can_receive_keyboard_focus() {
                    let keyboard = self.seat.get_keyboard().expect("seat has a keyboard");
                    keyboard.set_focus(self, Some(layer.wl_surface().clone()), serial);
                }
            } else {
                let clicked = self.space.element_under(at).map(|(w, _)| w.clone());
                if !self.mouse_grab(button, at, clicked.as_ref(), serial) {
                    // Click-to-focus; clicking the background clears focus.
                    self.focus_window(clicked);
                }
            }
        }
        pointer.button(self, &ButtonEvent { button, state: button_state, serial, time });
        pointer.frame(self);
    }

    /// SUPER+left-drag moves a window and SUPER+right-drag resizes it from
    /// the nearest corner; a tab shows its window (and drags it out);
    /// dragging the gap between tiles resizes them.
    /// True if a drag started (the click is the compositor's).
    fn mouse_grab(&mut self, button: u32, at: Point<f64, Logical>, clicked: Option<&Window>, serial: Serial) -> bool {
        const BTN_LEFT: u32 = 0x110;
        const BTN_RIGHT: u32 = 0x111;
        let start = GrabStartData { focus: None, button, location: at };
        if self.super_held() && (button == BTN_LEFT || button == BTN_RIGHT) {
            let Some(w) = clicked.filter(|w| !self.fullscreen.contains(w)).cloned() else { return false };
            self.focus_window(Some(w.clone()));
            if button == BTN_LEFT {
                self.start_move(w, start, serial);
            } else {
                let rect = self.space.element_geometry(&w).unwrap_or_default();
                self.start_resize(w, nearest_corner(rect, at), start, serial);
            }
            return true;
        }
        if button == BTN_LEFT && clicked.is_none() {
            // A tab: show it, and drag it if the pointer moves.
            if let Some(tab) = self.tab_at(at) {
                self.focus_window(Some(tab.clone()));
                self.start_move(tab, start, serial);
                return true;
            }
            if let Some((w, edges)) = self.gap_at(at) {
                self.start_tile_resize(&w, edges, start, serial);
                return true;
            }
        }
        false
    }

    /// Is SUPER held (or what stands in for it, with --mod ctrl-alt)?
    fn super_held(&self) -> bool {
        let Some(keyboard) = self.seat.get_keyboard() else { return false };
        let held = Mods::from(&keyboard.modifier_state());
        match self.keyboard.logo_remap {
            Some(m) => {
                (!m.ctrl || held.ctrl) && (!m.alt || held.alt) && (!m.shift || held.shift) && (!m.logo || held.logo)
            }
            None => held.logo,
        }
    }

    /// One key event from any backend (also driven directly by tests).
    pub fn on_key(&mut self, keycode: smithay::input::keyboard::Keycode, key_state: KeyState, time: u32) {
        self.activity();
        let serial = SERIAL_COUNTER.next_serial();
        let keyboard = self.seat.get_keyboard().expect("seat has a keyboard");

        let action =
            keyboard.input::<Option<Action>, _>(self, keycode, key_state, serial, time, |state, mods, handle| {
                // The unshifted symbol, so SUPER+SHIFT+1 is seen as "1", and
                // shortcuts keep working on non-Latin layouts.
                let sym = handle.raw_latin_sym_or_raw_current_sym().unwrap_or_else(|| handle.modified_sym());
                let pressed = key_state == KeyState::Pressed;

                // Ctrl+Alt+Fn (the keymap turns it into XF86Switch_VT_n):
                // always works, even locked, so you can get back to a console.
                use smithay::input::keyboard::keysyms::{KEY_XF86Switch_VT_1, KEY_XF86Switch_VT_12};
                let raw = handle.modified_sym().raw();
                if (KEY_XF86Switch_VT_1..=KEY_XF86Switch_VT_12).contains(&raw) && pressed && state.vt_switch.is_some() {
                    state.swallowed.push(keycode);
                    let vt = (raw - KEY_XF86Switch_VT_1 + 1) as i32;
                    return FilterResult::Intercept(Some(Action::SwitchVt(vt)));
                }

                // Locked: every key goes to the lock screen, no shortcuts.
                // Settings recording a new shortcut: it gets the keys.
                if state.is_locked() || state.shortcuts_paused.is_some() {
                    return FilterResult::Forward;
                }
                // While a switcher is open it owns the keyboard.
                if state.switcher.is_some() {
                    return state.switcher_key(sym, pressed, Mods::from(mods), keycode);
                }

                // SUPER on its own opens the launcher, on release, and only if no
                // other key was pressed while it was held.
                if is_logo_key(sym) {
                    if pressed {
                        state.logo_tap_pending = true;
                    } else if std::mem::take(&mut state.logo_tap_pending) {
                        if let Some(action) = state.bindings.logo_tap().cloned() {
                            return FilterResult::Intercept(Some(action));
                        }
                    }
                    return FilterResult::Forward;
                }

                if pressed {
                    state.logo_tap_pending = false;
                    if let Some(action) = state.bindings.lookup(Mods::from(mods), sym).cloned() {
                        state.shortcut_mods = Mods::from(mods);
                        state.swallowed.push(keycode);
                        return FilterResult::Intercept(Some(action));
                    }
                } else if let Some(i) = state.swallowed.iter().position(|k| *k == keycode) {
                    // The client never saw the press, so it must not see the release.
                    state.swallowed.swap_remove(i);
                    return FilterResult::Intercept(None);
                }
                FilterResult::Forward
            });

        if let Some(Some(action)) = action {
            tracing::debug!(?action, "shortcut");
            self.run_action(action);
        }
    }
}

impl WhiteRose {
    /// Keys while ALT+TAB / SUPER+ALT+TAB is open. Releasing the modifiers
    /// that opened it focuses the selection.
    fn switcher_key(
        &mut self,
        sym: smithay::input::keyboard::Keysym,
        pressed: bool,
        mods: Mods,
        keycode: smithay::input::keyboard::Keycode,
    ) -> smithay::input::keyboard::FilterResult<Option<Action>> {
        use smithay::input::keyboard::{keysyms as k, FilterResult};
        let Some((_, held)) = self.switcher.as_ref() else { return FilterResult::Forward };
        let held = *held;
        let raw = sym.raw();

        if !pressed {
            // Keep the swallowed-release bookkeeping straight.
            if let Some(i) = self.swallowed.iter().position(|c| *c == keycode) {
                self.swallowed.swap_remove(i);
            }
            let still_held = (!held.logo || mods.logo) && (!held.alt || mods.alt) && (!held.ctrl || mods.ctrl);
            if !still_held {
                self.commit_switcher();
            }
            // Modifier releases reach clients so their modifier state stays right.
            return FilterResult::Forward;
        }

        let is_modifier = matches!(
            raw,
            k::KEY_Alt_L
                | k::KEY_Alt_R
                | k::KEY_Super_L
                | k::KEY_Super_R
                | k::KEY_Control_L
                | k::KEY_Control_R
                | k::KEY_Shift_L
                | k::KEY_Shift_R
                | k::KEY_Meta_L
                | k::KEY_Meta_R
        );
        if is_modifier {
            return FilterResult::Forward;
        }

        let Some((s, _)) = self.switcher.as_mut() else { return FilterResult::Forward };
        match raw {
            k::KEY_Tab | k::KEY_ISO_Left_Tab if mods.shift => s.prev(),
            k::KEY_Tab | k::KEY_ISO_Left_Tab | k::KEY_Right | k::KEY_Down => s.next(),
            k::KEY_Left | k::KEY_Up => s.prev(),
            k::KEY_grave if s.mode == crate::switcher::Mode::Apps => s.next_window(),
            k::KEY_q => {
                let targets = s.close_targets();
                for w in self.workspaces.mru().iter().filter(|w| targets.contains(&self.window_id(w))) {
                    if let Some(t) = w.toplevel() {
                        t.send_close();
                    }
                }
            }
            k::KEY_Escape => self.cancel_switcher(),
            k::KEY_Return | k::KEY_KP_Enter => self.commit_switcher(),
            _ => {}
        }
        self.switcher_sync();
        // No key typed while switching reaches the focused app.
        self.swallowed.push(keycode);
        FilterResult::Intercept(None)
    }
}
