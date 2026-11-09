//! Input handling: shortcuts first, then forwarding to clients.
//! Pointer handling follows Smithay's smallvil example (MIT).

use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, InputBackend, InputEvent, KeyState,
        KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent,
    },
    input::{
        keyboard::FilterResult,
        pointer::{AxisFrame, ButtonEvent, MotionEvent},
    },
    utils::SERIAL_COUNTER,
};

use crate::{
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
                let serial = SERIAL_COUNTER.next_serial();
                let pointer = self.seat.get_pointer().expect("seat has a pointer");
                let under = self.surface_under(pos);
                pointer.motion(self, under, &MotionEvent { location: pos, serial, time: event.time_msec() });
                pointer.frame(self);
            }
            InputEvent::PointerButton { event, .. } => {
                self.logo_tap_pending = false;
                let pointer = self.seat.get_pointer().expect("seat has a pointer");
                let serial = SERIAL_COUNTER.next_serial();
                let button_state = event.state();

                if button_state == ButtonState::Pressed && !pointer.is_grabbed() {
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
                        // Click-to-focus; clicking the background clears focus.
                        self.focus_window(clicked);
                    }
                }
                pointer.button(
                    self,
                    &ButtonEvent { button: event.button_code(), state: button_state, serial, time: event.time_msec() },
                );
                pointer.frame(self);
            }
            InputEvent::PointerAxis { event, .. } => {
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

    fn on_key(&mut self, keycode: smithay::input::keyboard::Keycode, key_state: KeyState, time: u32) {
        let serial = SERIAL_COUNTER.next_serial();
        let keyboard = self.seat.get_keyboard().expect("seat has a keyboard");

        let action =
            keyboard.input::<Option<Action>, _>(self, keycode, key_state, serial, time, |state, mods, handle| {
                // The unshifted symbol, so SUPER+SHIFT+1 is seen as "1", and
                // shortcuts keep working on non-Latin layouts.
                let sym = handle.raw_latin_sym_or_raw_current_sym().unwrap_or_else(|| handle.modified_sym());
                let pressed = key_state == KeyState::Pressed;

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
