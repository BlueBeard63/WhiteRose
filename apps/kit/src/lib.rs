//! What WhiteRose's apps share (`import WhiteRose.Kit` in QML):
//! - `Desktop`: the session's settings, live (appearance, and any other);
//! - `Theme`: the desktop's colours and type, following Settings > Appearance;
//! - controls (Button, Toggle, Field, Choice, ...) and `AppWindow`, the
//!   window frame with the desktop's title buttons.
//!
//! An app depends on this crate and links it with `extern crate whiterose_kit;`.

pub mod daemon;
pub mod desktop;
pub mod icons;
pub mod sun;
