//! WhiteRose compositor library. The binary (`whiterose-wm`) is a thin
//! wrapper; integration tests drive the same state directly.

pub mod clipboard;
pub mod clipboard_capture;
pub mod cursor;
pub mod daemon_client;
pub mod display_config;
pub mod drm;
pub mod foreign_toplevel;
pub mod grabs;
mod handlers;
pub mod headless;
mod host_inhibit;
pub mod idle;
pub mod input;
pub mod ipc;
pub mod keybind;
pub mod keyboard;
pub mod layout;
pub mod monitor_id;
pub mod night_light;
pub mod outputs;
pub mod protocols;
pub mod ready;
pub mod render;
pub mod rules;
pub mod screenshot;
pub mod session_lock;
pub mod shell_supervisor;
pub mod snapping;
pub mod state;
pub mod switcher;
pub mod tab_strip;
pub mod tiling;
pub mod toplevel_export;
pub mod window_rules;
pub mod window_state;
pub mod winit;
pub mod workspace;

pub use state::WhiteRose;
