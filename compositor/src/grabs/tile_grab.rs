//! Moving and resizing tiles with the mouse.
//!
//! [`TileMoveGrab`]: the tile follows the pointer while the others keep
//! their places, and a preview shows where it will land: dropped on another
//! tile it swaps with it (the middle) or splits it (near an edge); dropped
//! outside the tiled area it floats there.
//!
//! [`TileResizeGrab`]: moves the edges of a tile, and the splits they sit
//! on, so the tiles across them follow.

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

use super::{move_grab::keep_below, resize_grab::ResizeEdge};
use crate::{
    layout::{Dir, Tiling},
    WhiteRose,
};

/// The tile edges a window-border resize moves.
pub fn edge_dirs(edges: ResizeEdge) -> Vec<Dir> {
    [
        (ResizeEdge::LEFT, Dir::Left),
        (ResizeEdge::RIGHT, Dir::Right),
        (ResizeEdge::TOP, Dir::Up),
        (ResizeEdge::BOTTOM, Dir::Down),
    ]
    .into_iter()
    .filter(|(e, _)| edges.contains(*e))
    .map(|(_, d)| d)
    .collect()
}

/// The corner of `rect` nearest `pos`, as the edges SUPER+right-drag moves.
pub fn nearest_corner(rect: smithay::utils::Rectangle<i32, Logical>, pos: Point<f64, Logical>) -> ResizeEdge {
    let centre = rect.loc.to_f64() + rect.size.to_f64().downscale(2.0).to_point();
    let x = if pos.x < centre.x { ResizeEdge::LEFT } else { ResizeEdge::RIGHT };
    let y = if pos.y < centre.y { ResizeEdge::TOP } else { ResizeEdge::BOTTOM };
    x | y
}

/// The pointer-grab methods these grabs pass straight through.
macro_rules! pass_through {
    () => {
        fn relative_motion(
            &mut self,
            data: &mut WhiteRose,
            handle: &mut PointerInnerHandle<'_, WhiteRose>,
            focus: Option<(WlSurface, Point<f64, Logical>)>,
            event: &RelativeMotionEvent,
        ) {
            handle.relative_motion(data, focus, event);
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
    };
}

pub struct TileMoveGrab {
    pub start_data: PointerGrabStartData<WhiteRose>,
    pub window: Window,
    pub initial_window_location: Point<i32, Logical>,
}

impl PointerGrab<WhiteRose> for TileMoveGrab {
    fn motion(
        &mut self,
        data: &mut WhiteRose,
        handle: &mut PointerInnerHandle<'_, WhiteRose>,
        _focus: Option<(WlSurface, Point<f64, Logical>)>,
        event: &MotionEvent,
    ) {
        handle.motion(data, None, event);
        let delta = event.location - self.start_data.location;
        let mut at = (self.initial_window_location.to_f64() + delta).to_i32_round();
        if let Some(o) = data.output_at(event.location) {
            at.y = keep_below(at.y, data.usable_area_on(&o).loc.y);
        }
        data.space.map_element(self.window.clone(), at, false);
        data.show_drop_preview(&self.window, event.location);
    }

    fn button(&mut self, data: &mut WhiteRose, handle: &mut PointerInnerHandle<'_, WhiteRose>, event: &ButtonEvent) {
        handle.button(data, event);
        if !handle.current_pressed().contains(&self.start_data.button) {
            let at = handle.current_location();
            handle.unset_grab(self, data, event.serial, event.time, true);
            data.drop_tile(&self.window, at);
        }
    }

    pass_through!();

    fn unset(&mut self, data: &mut WhiteRose) {
        data.end_tile_drag();
    }
}

pub struct TileResizeGrab {
    pub start_data: PointerGrabStartData<WhiteRose>,
    pub window: Window,
    /// The edges being dragged.
    pub edges: Vec<Dir>,
    /// The tiles as they were when the drag started: each motion resizes
    /// from these, so the result follows the pointer exactly.
    pub snapshot: Tiling<Window>,
}

impl PointerGrab<WhiteRose> for TileResizeGrab {
    fn motion(
        &mut self,
        data: &mut WhiteRose,
        handle: &mut PointerInnerHandle<'_, WhiteRose>,
        _focus: Option<(WlSurface, Point<f64, Logical>)>,
        event: &MotionEvent,
    ) {
        handle.motion(data, None, event);
        let delta = (event.location - self.start_data.location).to_i32_round::<i32>();
        // Outwards is positive: left and up are negative on screen.
        let moves: Vec<(Dir, i32)> = self
            .edges
            .iter()
            .map(|d| {
                (
                    *d,
                    match d {
                        Dir::Left => -delta.x,
                        Dir::Right => delta.x,
                        Dir::Up => -delta.y,
                        Dir::Down => delta.y,
                    },
                )
            })
            .collect();
        data.resize_tile(&self.snapshot, &self.window, &moves);
    }

    fn button(&mut self, data: &mut WhiteRose, handle: &mut PointerInnerHandle<'_, WhiteRose>, event: &ButtonEvent) {
        handle.button(data, event);
        if !handle.current_pressed().contains(&self.start_data.button) {
            handle.unset_grab(self, data, event.serial, event.time, true);
        }
    }

    pass_through!();

    fn unset(&mut self, _data: &mut WhiteRose) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use smithay::utils::Rectangle;

    #[test]
    fn border_edges_become_tile_edges() {
        assert_eq!(edge_dirs(ResizeEdge::RIGHT), vec![Dir::Right]);
        assert_eq!(edge_dirs(ResizeEdge::TOP_LEFT), vec![Dir::Left, Dir::Up]);
        assert_eq!(edge_dirs(ResizeEdge::BOTTOM_RIGHT), vec![Dir::Right, Dir::Down]);
    }

    #[test]
    fn the_nearest_corner_picks_the_edges() {
        let r = Rectangle::new((100, 100).into(), (200, 100).into());
        assert_eq!(nearest_corner(r, (120.0, 110.0).into()), ResizeEdge::TOP_LEFT);
        assert_eq!(nearest_corner(r, (290.0, 190.0).into()), ResizeEdge::BOTTOM_RIGHT);
        assert_eq!(nearest_corner(r, (290.0, 110.0).into()), ResizeEdge::TOP_RIGHT);
    }
}
