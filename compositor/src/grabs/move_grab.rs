// Adapted from Smithay's smallvil example (MIT, (c) Victor Berger and Victoria Brekenfeld).
use crate::WhiteRose;
use smithay::{
    desktop::Window,
    input::pointer::{
        AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent,
        GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent, GestureSwipeEndEvent,
        GestureSwipeUpdateEvent, GrabStartData as PointerGrabStartData, MotionEvent, PointerGrab, PointerInnerHandle,
        RelativeMotionEvent,
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point},
};

/// A window's top at `y`, kept at or below `top` (the top of the area the
/// bar doesn't reserve on the output being dragged over).
pub fn keep_below(y: i32, top: i32) -> i32 {
    y.max(top)
}

pub struct MoveSurfaceGrab {
    pub start_data: PointerGrabStartData<WhiteRose>,
    pub window: Window,
    pub initial_window_location: Point<i32, Logical>,
}

impl PointerGrab<WhiteRose> for MoveSurfaceGrab {
    fn motion(
        &mut self,
        data: &mut WhiteRose,
        handle: &mut PointerInnerHandle<'_, WhiteRose>,
        _focus: Option<(WlSurface, Point<f64, Logical>)>,
        event: &MotionEvent,
    ) {
        // While the grab is active, no client has pointer focus
        handle.motion(data, None, event);

        let delta = event.location - self.start_data.location;
        let mut at = (self.initial_window_location.to_f64() + delta).to_i32_round();
        // Never under the bar: its title bar would be covered there and
        // the window couldn't be grabbed again.
        if let Some(o) = data.output_at(event.location) {
            at.y = keep_below(at.y, data.usable_area_on(&o).loc.y);
        }
        data.space.map_element(self.window.clone(), at, true);
        data.island_sync(); // dragged under (or away from) the island
    }

    fn relative_motion(
        &mut self,
        data: &mut WhiteRose,
        handle: &mut PointerInnerHandle<'_, WhiteRose>,
        focus: Option<(WlSurface, Point<f64, Logical>)>,
        event: &RelativeMotionEvent,
    ) {
        handle.relative_motion(data, focus, event);
    }

    fn button(&mut self, data: &mut WhiteRose, handle: &mut PointerInnerHandle<'_, WhiteRose>, event: &ButtonEvent) {
        handle.button(data, event);
        // Released the button that started it (left for a title bar, right
        // or left for SUPER+drag).
        if !handle.current_pressed().contains(&self.start_data.button) {
            handle.unset_grab(self, data, event.serial, event.time, true);
            // Dropped on a tiled output, a window joins its tiles.
            data.retile();
        }
    }

    fn axis(&mut self, data: &mut WhiteRose, handle: &mut PointerInnerHandle<'_, WhiteRose>, details: AxisFrame) {
        handle.axis(data, details)
    }

    fn frame(&mut self, data: &mut WhiteRose, handle: &mut PointerInnerHandle<'_, WhiteRose>) {
        handle.frame(data);
    }

    fn gesture_swipe_begin(
        &mut self,
        data: &mut WhiteRose,
        handle: &mut PointerInnerHandle<'_, WhiteRose>,
        event: &GestureSwipeBeginEvent,
    ) {
        handle.gesture_swipe_begin(data, event)
    }

    fn gesture_swipe_update(
        &mut self,
        data: &mut WhiteRose,
        handle: &mut PointerInnerHandle<'_, WhiteRose>,
        event: &GestureSwipeUpdateEvent,
    ) {
        handle.gesture_swipe_update(data, event)
    }

    fn gesture_swipe_end(
        &mut self,
        data: &mut WhiteRose,
        handle: &mut PointerInnerHandle<'_, WhiteRose>,
        event: &GestureSwipeEndEvent,
    ) {
        handle.gesture_swipe_end(data, event)
    }

    fn gesture_pinch_begin(
        &mut self,
        data: &mut WhiteRose,
        handle: &mut PointerInnerHandle<'_, WhiteRose>,
        event: &GesturePinchBeginEvent,
    ) {
        handle.gesture_pinch_begin(data, event)
    }

    fn gesture_pinch_update(
        &mut self,
        data: &mut WhiteRose,
        handle: &mut PointerInnerHandle<'_, WhiteRose>,
        event: &GesturePinchUpdateEvent,
    ) {
        handle.gesture_pinch_update(data, event)
    }

    fn gesture_pinch_end(
        &mut self,
        data: &mut WhiteRose,
        handle: &mut PointerInnerHandle<'_, WhiteRose>,
        event: &GesturePinchEndEvent,
    ) {
        handle.gesture_pinch_end(data, event)
    }

    fn gesture_hold_begin(
        &mut self,
        data: &mut WhiteRose,
        handle: &mut PointerInnerHandle<'_, WhiteRose>,
        event: &GestureHoldBeginEvent,
    ) {
        handle.gesture_hold_begin(data, event)
    }

    fn gesture_hold_end(
        &mut self,
        data: &mut WhiteRose,
        handle: &mut PointerInnerHandle<'_, WhiteRose>,
        event: &GestureHoldEndEvent,
    ) {
        handle.gesture_hold_end(data, event)
    }

    fn start_data(&self) -> &PointerGrabStartData<WhiteRose> {
        &self.start_data
    }

    fn unset(&mut self, _data: &mut WhiteRose) {}
}

#[cfg(test)]
mod tests {
    #[test]
    fn stays_below_the_bar() {
        assert_eq!(super::keep_below(-40, 32), 32);
        assert_eq!(super::keep_below(10, 32), 32);
        assert_eq!(super::keep_below(200, 32), 200);
        // A second output below the first: its own top.
        assert_eq!(super::keep_below(1100, 1112), 1112);
    }
}
