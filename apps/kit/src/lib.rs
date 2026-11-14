//! What WhiteRose's apps share (`import WhiteRose.Kit` in QML):
//! - `Desktop`: the session's settings, live (appearance, and any other);
//! - `Theme`: the desktop's colours and type, following Settings > Appearance;
//! - controls (Button, Toggle, Field, Choice, ...) and `AppWindow`, the
//!   window frame with the desktop's title buttons (resting on maximize
//!   asks the compositor for its snap layout picker).
//!
//! An app depends on this crate and links it with `extern crate whiterose_kit;`.

pub mod compositor;
pub mod daemon;
pub mod desktop;
pub mod icons;
#[cfg(test)]
#[path = "../../scratch.rs"]
mod scratch;
pub mod sun;
