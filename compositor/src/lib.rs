//! WhiteRose compositor library. The binary (`whiterose-wm`) is a thin
//! wrapper; integration tests drive the same state directly.

pub mod foreign_toplevel;
pub mod grabs;
mod host_inhibit;
mod handlers;
pub mod input;
pub mod ipc;
pub mod keybind;
pub mod protocols;
pub mod render;
pub mod screenshot;
pub mod state;
pub mod switcher;
pub mod toplevel_export;
pub mod window_state;
pub mod winit;
pub mod workspace;

pub use state::WhiteRose;
