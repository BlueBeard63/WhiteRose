//! The cursor over nothing (no window, layer or wallpaper under it).

mod common;

use common::*;
use smithay::input::pointer::{CursorIcon, CursorImageStatus};

/// An app's cursor (a resize arrow at its edge) doesn't stay once the
/// pointer leaves it for nothing, like a gap between windows: it's the
/// arrow again.
#[test]
fn over_nothing_the_cursor_is_the_arrow() {
    let mut h = Headless::new(true);
    h.state.cursor_status = CursorImageStatus::Named(CursorIcon::EwResize);
    h.state.on_pointer_motion((300.0, 300.0).into(), 1);
    assert_eq!(h.state.cursor_status, CursorImageStatus::default_named());
}
