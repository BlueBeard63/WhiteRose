//! WhiteRose compositor library. The binary (`whiterose-wm`) is a thin
//! wrapper; integration tests drive the same state directly.

pub mod grabs;
mod handlers;
pub mod input;
pub mod ipc;
pub mod keybind;
pub mod render;
pub mod screenshot;
pub mod state;
pub mod window_state;
pub mod winit;
pub mod workspace;

pub use state::WhiteRose;
