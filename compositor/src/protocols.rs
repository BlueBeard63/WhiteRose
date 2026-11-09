//! Protocols Smithay doesn't ship, generated from vendored XML.

/// hyprland-toplevel-export-v1 (BSD-3-Clause, (c) 2022 Vaxry; see the XML).
/// QuickShell's ScreencopyView captures single windows through this, using
/// handles from wlr-foreign-toplevel-management.
#[allow(non_upper_case_globals, non_camel_case_types, unused_imports, missing_docs, clippy::all)]
pub mod toplevel_export {
    use smithay::reexports::wayland_protocols_wlr::foreign_toplevel::v1::server::*;
    use smithay::reexports::wayland_server;
    use smithay::reexports::wayland_server::protocol::*;

    pub mod __interfaces {
        use smithay::reexports::wayland_protocols_wlr::foreign_toplevel::v1::server::__interfaces::*;
        use smithay::reexports::wayland_server::backend as wayland_backend;
        use smithay::reexports::wayland_server::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocols/hyprland-toplevel-export-v1.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_server_code!("protocols/hyprland-toplevel-export-v1.xml");
}
