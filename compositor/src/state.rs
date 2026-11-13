//! Compositor state. Structure follows Smithay's smallvil example (MIT).

use crate::win::WinExt;
use std::{ffi::OsString, sync::Arc};

use smithay::{
    backend::allocator::dmabuf::Dmabuf,
    desktop::{layer_map_for_output, LayerSurface, PopupManager, Space, Window, WindowSurfaceType},
    input::{keyboard::Keycode, Seat, SeatState},
    reexports::{
        calloop::{generic::Generic, ping::Ping, EventLoop, Interest, LoopHandle, LoopSignal, Mode, PostAction},
        wayland_server::{
            backend::{ClientData, ClientId, DisconnectReason},
            protocol::wl_surface::WlSurface,
            Display, DisplayHandle,
        },
    },
    utils::{Logical, Point, SERIAL_COUNTER},
    wayland::{
        compositor::{CompositorClientState, CompositorState},
        dmabuf::{DmabufGlobal, DmabufState},
        output::OutputManagerState,
        selection::data_device::DataDeviceState,
        shell::{
            wlr_layer::{Layer, WlrLayerShellState},
            xdg::XdgShellState,
        },
        shm::ShmState,
        socket::ListeningSocketSource,
    },
};

use crate::{
    ipc,
    keybind::{Action, Bindings},
    workspace::Workspaces,
};

pub const WORKSPACES: usize = 9;

/// Stable id given to each window for IPC (dock, switchers).
#[derive(Debug, Clone, Copy)]
pub struct WindowId(pub u64);

/// Imports a client GPU buffer into the renderer; returns whether it worked.
pub type DmabufImporter = Box<dyn FnMut(&Dmabuf) -> bool>;

pub struct WhiteRose {
    pub start_time: std::time::Instant,
    /// Where this session's whiterosed listens (passed to everything we start).
    pub daemon_socket: std::path::PathBuf,
    /// Connection to whiterosed (None while it's not running).
    pub daemon: Option<std::os::unix::net::UnixStream>,
    /// Requests made while it wasn't connected, sent when it connects.
    pub daemon_waiting: Vec<serde_json::Value>,
    /// Display settings: the saved layout, and one being tried out.
    pub displays: crate::display_config::Displays,
    pub display_trial: Option<crate::daemon_client::Trial>,
    /// The profile that placed the current layout, if any.
    pub display_profile: Option<String>,
    /// Every connected output (also the turned-off ones, which aren't in the space).
    pub outputs: Vec<smithay::output::Output>,
    /// What the backend can change about outputs.
    pub caps: crate::outputs::Capabilities,
    pub socket_name: OsString,
    pub display_handle: DisplayHandle,
    pub loop_signal: LoopSignal,
    /// What the process exits with (RESTART_CODE: start the session again).
    pub exit_code: i32,
    pub loop_handle: LoopHandle<'static, WhiteRose>,
    /// Shell IPC. None if the socket could not be created.
    pub ipc: Option<ipc::Server>,

    pub space: Space<Window>,
    pub workspaces: Workspaces<Window>,
    /// Where hidden windows were, so switching back restores them.
    pub hidden_at: Vec<(Window, Point<i32, Logical>)>,
    /// Floating geometry of maximized/fullscreen windows, for restoring.
    pub restore_geometry: Vec<(Window, smithay::utils::Rectangle<i32, Logical>)>,
    /// Fullscreen windows: at most one per output.
    pub fullscreen: Vec<Window>,
    /// The output each maximized/fullscreen window fills, so re-arranging
    /// or resizing outputs refits it there (not wherever it now overlaps).
    pub filling: Vec<(Window, smithay::output::Output)>,
    /// Minimised windows: hidden until focused again (dock, ALT+TAB).
    pub minimized: Vec<Window>,
    /// Tiled workspaces and the layout settings (see tiling.rs).
    pub tiling: crate::tiling::TilingState,
    /// Snapped floating windows (see snapping.rs).
    pub snap: crate::snapping::SnapState,
    /// `windows.rules`: applied as windows open (window_rules.rs).
    pub rules: crate::rules::Rules,
    /// `windows.scratchpads` and their windows (scratchpad_windows.rs).
    pub scratchpads: crate::scratchpad_windows::Scratchpads,
    /// Output and usable area last used to fit maximized/fullscreen windows.
    /// PrtSc pressed: capture on the next frame.
    pub screenshot_pending: bool,
    /// What the pending screenshot captures (region, pointer, save or copy).
    pub screenshot_shot: crate::screenshot::Shot,
    /// Worker threads report finished captures here.
    pub screenshot_tx: smithay::reexports::calloop::channel::Sender<crate::screenshot::Outcome>,
    /// Paths this compositor saved; the only files the shell may ask it to delete.
    pub saved_screenshots: Vec<std::path::PathBuf>,
    next_window_id: u64,
    /// The open switcher and the modifiers that keep it open (released = commit).
    pub switcher: Option<(crate::switcher::Switcher, crate::keybind::Mods)>,
    /// ALT+TAB lists only the active workspace's apps
    /// (`windows.appSwitcher.workspaces` "current"); SUPER+TAB always all.
    pub app_switcher_current_only: bool,
    /// Modifiers of the shortcut that triggered the current action.
    pub shortcut_mods: crate::keybind::Mods,
    /// Per output: (whole, usable) rectangles last used to fit them.
    pub fitted_to: Vec<(smithay::utils::Rectangle<i32, Logical>, smithay::utils::Rectangle<i32, Logical>)>,

    pub bindings: Bindings,
    /// Keys whose press was a shortcut; their release is swallowed too.
    pub swallowed: Vec<Keycode>,
    /// SUPER is down and nothing else has been pressed since.
    pub logo_tap_pending: bool,

    /// Wakes the backend to draw a frame. None in the compositor tests.
    pub redraw: Option<Ping>,
    /// The backend draws the screen, so it can be captured (screenshots).
    /// Not headless: it draws nothing.
    pub can_capture: bool,
    /// Imports a client's GPU buffer into the renderer. Set by the backend.
    pub dmabuf_importer: Option<DmabufImporter>,
    pub dmabuf_global: Option<DmabufGlobal>,

    // Smithay protocol state
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<WhiteRose>,
    pub data_device_state: DataDeviceState,
    pub layer_shell_state: WlrLayerShellState,
    pub dmabuf_state: DmabufState,
    pub foreign_toplevels: crate::foreign_toplevel::ForeignToplevelState,
    /// Window copies requested by clients (live previews), filled after frames.
    pub pending_exports: Vec<crate::toplevel_export::PendingCopy>,
    pub export_timer_armed: bool,
    /// The icon of a drag in progress (drawn under the pointer).
    pub dnd_icon: Option<WlSurface>,
    /// The pointer image clients asked for (drawn by backends that draw it).
    pub cursor_status: smithay::input::pointer::CursorImageStatus,
    /// True when the backend draws the pointer (DRM), so moving it redraws.
    pub draws_cursor: bool,
    /// Idle lock and screens off (see idle.rs).
    pub idle: crate::idle::Idle,
    pub idle_inhibit_state: smithay::wayland::idle_inhibit::IdleInhibitManagerState,
    /// "privacy.clipboardHistory": remember what's copied.
    pub clipboard_history_on: bool,
    /// App ids kept running when memory runs out (the `memory.protected`
    /// setting; the terminal always is): app_scope.rs.
    pub memory_protected: Vec<String>,
    /// Layouts, key repeat and the shortcut settings (see keyboard.rs).
    pub keyboard: crate::keyboard::KeyboardSettings,
    /// Mice and touchpads (pointer.rs), and the libinput devices they apply to.
    pub pointer: crate::pointer::PointerSettings,
    pub input_devices: Vec<smithay::reexports::input::Device>,
    /// Shake to find (shake.rs): how big the pointer is drawn.
    pub shake: crate::shake::Shake,
    /// The IPC client (Settings) recording a shortcut: shortcuts are off.
    pub shortcuts_paused: Option<u64>,
    /// The cursor theme and size we draw with (DRM), passed on to apps.
    pub cursor_theme: Option<(String, u32)>,
    pub cursor_shape_state: smithay::wayland::cursor_shape::CursorShapeManagerState,
    /// Switches to another console (DRM backend only).
    pub vt_switch: Option<Box<dyn FnMut(i32)>>,
    pub night_light: crate::night_light::NightLight,
    pub session_lock_state: smithay::wayland::session_lock::SessionLockManagerState,
    /// wp_fractional_scale_v1 and wp_viewporter: clients draw at exactly
    /// 125%, 150%, ... instead of rounding up to 200% and being scaled down.
    pub fractional_scale_state: smithay::wayland::fractional_scale::FractionalScaleManagerState,
    pub viewporter_state: smithay::wayland::viewporter::ViewporterState,
    pub lock: crate::session_lock::SessionLock,
    /// Restart bookkeeping per supervised process ("shell", "daemon").
    pub restarts: std::collections::HashMap<&'static str, crate::shell_supervisor::RestartPolicy>,
    /// Clipboard history (saved under clipboard_dir).
    pub clipboard: crate::clipboard::History,
    pub clipboard_dir: std::path::PathBuf,
    pub clipboard_tx: smithay::reexports::calloop::channel::Sender<crate::clipboard_capture::Captured>,
    pub popups: PopupManager,
    /// XWayland: X11 apps (xwayland.rs).
    pub xwayland: crate::xwayland::XState,
    pub seat: Seat<Self>,
}

impl WhiteRose {
    pub fn new(event_loop: &mut EventLoop<'static, Self>, display: Display<Self>, bindings: Bindings) -> Self {
        let dh = display.handle();
        let xwayland = crate::xwayland::XState::new(&dh);

        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let mut seat_state = SeatState::new();
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        let layer_shell_state = WlrLayerShellState::new::<Self>(&dh);
        let session_lock_state = smithay::wayland::session_lock::SessionLockManagerState::new::<Self, _>(&dh, |_| true);
        let fractional_scale_state = smithay::wayland::fractional_scale::FractionalScaleManagerState::new::<Self>(&dh);
        let viewporter_state = smithay::wayland::viewporter::ViewporterState::new::<Self>(&dh);
        let cursor_shape_state = smithay::wayland::cursor_shape::CursorShapeManagerState::new::<Self>(&dh);
        let idle_inhibit_state = smithay::wayland::idle_inhibit::IdleInhibitManagerState::new::<Self>(&dh);
        let foreign_toplevels = crate::foreign_toplevel::ForeignToplevelState::new(&dh);
        crate::toplevel_export::init(&dh);

        let mut seat: Seat<Self> = seat_state.new_wl_seat(&dh, "seat0");
        // TODO: track keyboard/pointer hotplug once the libinput backend exists.
        seat.add_keyboard(Default::default(), 300, 30).expect("default keymap");
        seat.add_pointer();

        let socket_name = Self::init_wayland_listener(display, event_loop);
        let (clipboard_tx, clipboard_rx) = smithay::reexports::calloop::channel::channel();
        event_loop
            .handle()
            .insert_source(clipboard_rx, |event, _, state| {
                if let smithay::reexports::calloop::channel::Event::Msg(captured) = event {
                    state.clipboard_captured(captured);
                }
            })
            .expect("insert clipboard channel");
        let clipboard_dir = std::env::var_os("WHITEROSE_CLIPBOARD_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(crate::clipboard::history_dir);
        let clipboard = crate::clipboard::load(&clipboard_dir);
        let (screenshot_tx, screenshot_rx) = smithay::reexports::calloop::channel::channel();
        event_loop
            .handle()
            .insert_source(screenshot_rx, |event, _, state| {
                if let smithay::reexports::calloop::channel::Event::Msg(outcome) = event {
                    state.screenshot_done(outcome);
                }
            })
            .expect("insert screenshot channel");
        let ipc_path = std::env::var_os("XDG_RUNTIME_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join(format!("whiterose-{}.sock", socket_name.to_string_lossy()));
        // whiterosed's socket for this session (it creates it; we only say where).
        let daemon_socket = ipc_path.with_file_name(format!("whiterosed-{}.sock", socket_name.to_string_lossy()));
        let ipc = ipc::Server::bind(&ipc_path, &event_loop.handle())
            .map_err(|err| tracing::warn!(%err, "shell ipc disabled"))
            .ok();

        Self {
            start_time: std::time::Instant::now(),
            daemon_socket,
            daemon: None,
            daemon_waiting: Vec::new(),
            displays: Default::default(),
            display_trial: None,
            display_profile: None,
            outputs: Vec::new(),
            caps: crate::outputs::Capabilities {
                modes: false,
                rotation: false,
                position: false,
                disable: false,
                vrr: false,
                pointer: false,
            },
            socket_name,
            display_handle: dh,
            loop_signal: event_loop.get_signal(),
            exit_code: 0,
            loop_handle: event_loop.handle(),
            ipc,
            space: Space::default(),
            workspaces: Workspaces::new(WORKSPACES),
            hidden_at: Vec::new(),
            restore_geometry: Vec::new(),
            fullscreen: Vec::new(),
            filling: Vec::new(),
            minimized: Vec::new(),
            tiling: Default::default(),
            snap: Default::default(),
            fitted_to: Vec::new(),
            next_window_id: 1,
            switcher: None,
            app_switcher_current_only: false,
            shortcut_mods: crate::keybind::Mods::NONE,
            screenshot_pending: false,
            screenshot_shot: Default::default(),
            screenshot_tx,
            saved_screenshots: Vec::new(),
            bindings,
            swallowed: Vec::new(),
            logo_tap_pending: false,
            redraw: None,
            can_capture: false,
            dmabuf_importer: None,
            dmabuf_global: None,
            compositor_state,
            xdg_shell_state,
            shm_state,
            output_manager_state,
            seat_state,
            data_device_state,
            layer_shell_state,
            // The global is created by the backend, which knows the GPU formats.
            dmabuf_state: DmabufState::new(),
            foreign_toplevels,
            pending_exports: Vec::new(),
            export_timer_armed: false,
            dnd_icon: None,
            night_light: Default::default(),
            cursor_status: smithay::input::pointer::CursorImageStatus::default_named(),
            draws_cursor: false,
            cursor_theme: None,
            keyboard: crate::keyboard::KeyboardSettings {
                repeat_delay: 300,
                repeat_rate: 30,
                terminal: "ptyxis --standalone".into(),
                ..Default::default()
            },
            pointer: Default::default(),
            input_devices: Vec::new(),
            shake: Default::default(),
            shortcuts_paused: None,
            idle: Default::default(),
            idle_inhibit_state,
            clipboard_history_on: true,
            memory_protected: Vec::new(),
            rules: Default::default(),
            scratchpads: Default::default(),
            cursor_shape_state,
            vt_switch: None,
            session_lock_state,
            fractional_scale_state,
            viewporter_state,
            lock: Default::default(),
            restarts: Default::default(),
            clipboard,
            clipboard_dir,
            clipboard_tx,
            popups: PopupManager::default(),
            xwayland,
            seat,
        }
    }

    fn init_wayland_listener(display: Display<Self>, event_loop: &mut EventLoop<'static, Self>) -> OsString {
        let listening_socket =
            ListeningSocketSource::new_auto().expect("create wayland socket (is XDG_RUNTIME_DIR set?)");
        let socket_name = listening_socket.socket_name().to_os_string();
        let handle = event_loop.handle();

        handle
            .insert_source(listening_socket, |stream, _, state| {
                if let Err(err) = state.display_handle.insert_client(stream, Arc::new(ClientState::default())) {
                    tracing::warn!(?err, "failed to add client");
                }
            })
            .expect("insert wayland socket source");

        handle
            .insert_source(Generic::new(display, Interest::READ, Mode::Level), |_, display, state| {
                // Safety: the display is owned by this source and never dropped while it runs.
                unsafe {
                    display.get_mut().dispatch_clients(state).expect("dispatch clients");
                }
                // Flush replies right away instead of waiting for the next frame,
                // so clients never stall on an idle compositor.
                let _ = state.display_handle.flush_clients();
                Ok(PostAction::Continue)
            })
            .expect("insert display source");

        socket_name
    }

    /// Runs after every event-loop iteration. Sends clients everything queued
    /// for them, whatever produced it: host or libinput input, IPC, timers,
    /// frame callbacks after a redraw. Without this, events not caused by a
    /// client request wait until some client happens to send one, so clicks
    /// and animation frames arrive late or seemingly at random.
    pub fn after_dispatch(&mut self) {
        self.sync_x11();
        let _ = self.display_handle.flush_clients();
    }

    /// Asks the backend for a frame. Called whenever something visible changes;
    /// an idle desktop draws nothing.
    pub fn schedule_redraw(&self) {
        if let Some(ping) = &self.redraw {
            ping.ping();
        }
    }

    /// The surface under the pointer, respecting layer order:
    /// overlay, top (bar, panels), windows, bottom, background.
    pub fn surface_under(&self, pos: Point<f64, Logical>) -> Option<(WlSurface, Point<f64, Logical>)> {
        if self.is_locked() {
            return self.lock_surface_under(pos);
        }
        let output_hit = self
            .space
            .outputs()
            .find_map(|o| self.space.output_geometry(o).filter(|g| g.to_f64().contains(pos)).map(|g| (o, g)));

        let layer_hit = |layer: Layer| {
            let (output, geo) = output_hit?;
            let map = layer_map_for_output(output);
            let local = pos - geo.loc.to_f64();
            // The topmost surface that takes input here, not just the
            // topmost whose rectangle covers it: a mostly click-through
            // layer (its input region is a few shapes) over another, like the
            // island's window over the screenshot toast, lets clicks through.
            let hit = map.layers_on(layer).rev().find_map(|surface| {
                let layer_geo = map.layer_geometry(surface)?;
                surface
                    .surface_under(local - layer_geo.loc.to_f64(), WindowSurfaceType::ALL)
                    .map(|(s, p)| (s, (p + layer_geo.loc + geo.loc).to_f64()))
            });
            hit
        };
        let window_hit = || {
            self.window_under(pos).and_then(|(window, location)| {
                window
                    .surface_under(pos - location.to_f64(), WindowSurfaceType::ALL)
                    .map(|(s, p)| (s, (p + location).to_f64()))
            })
        };

        if self.output_at(pos).is_some_and(|o| self.fullscreen_on(&o).is_some()) {
            return layer_hit(Layer::Overlay).or_else(window_hit);
        }
        layer_hit(Layer::Overlay)
            .or_else(|| layer_hit(Layer::Top))
            .or_else(window_hit)
            .or_else(|| layer_hit(Layer::Bottom))
            .or_else(|| layer_hit(Layer::Background))
    }

    /// The window under `pos` and where it's drawn (its surface's origin),
    /// like `Space::element_under`, except that a tiled window only takes
    /// the pointer inside its tile (`clip_of`); its popups take it anywhere.
    pub fn window_under(&self, pos: Point<f64, Logical>) -> Option<(&Window, Point<i32, Logical>)> {
        use smithay::desktop::space::SpaceElement;
        self.space.elements().rev().find_map(|w| {
            // A scratchpad on its way out lets the pointer through.
            if self.scratchpads.sliding_out(w) || !self.space.element_bbox(w)?.to_f64().contains(pos) {
                return None;
            }
            let at = self.space.element_location(w)? - w.geometry().loc;
            let local = pos - at.to_f64();
            if self.clip_of(w).is_some_and(|c| !c.to_f64().contains(pos)) {
                return w.surface_under(local, WindowSurfaceType::POPUP).map(|_| (w, at));
            }
            w.is_in_input_region(&local).then_some((w, at))
        })
    }

    /// The top or overlay layer surface (bar, panel, launcher) under `pos`.
    pub fn top_layer_at(&self, pos: Point<f64, Logical>) -> Option<LayerSurface> {
        let (output, geo) = self
            .space
            .outputs()
            .find_map(|o| self.space.output_geometry(o).filter(|g| g.to_f64().contains(pos)).map(|g| (o, g)))?;
        let map = layer_map_for_output(output);
        let local = pos - geo.loc.to_f64();
        // Only where the panel takes input: the bar's window is mostly
        // transparent and click-through (its input region covers just the
        // visible shapes), and a click there belongs to what's underneath.
        [Layer::Overlay, Layer::Top].into_iter().find_map(|l| {
            map.layers_on(l).rev().find_map(|layer| {
                let layer_geo = map.layer_geometry(layer)?;
                layer.surface_under(local - layer_geo.loc.to_f64(), WindowSurfaceType::ALL).map(|_| layer.clone())
            })
        })
    }

    /// The part of the first output not reserved by panels (e.g. the bar).
    pub fn usable_area(&self) -> smithay::utils::Rectangle<i32, Logical> {
        self.space.outputs().next().map(|o| self.usable_area_on(o)).unwrap_or_default()
    }

    /// The part of `output` not reserved by panels, in global coordinates.
    pub fn usable_area_on(&self, output: &smithay::output::Output) -> smithay::utils::Rectangle<i32, Logical> {
        let origin = self.space.output_geometry(output).map(|g| g.loc).unwrap_or_default();
        let zone = layer_map_for_output(output).non_exclusive_zone();
        smithay::utils::Rectangle::new(origin + zone.loc, zone.size)
    }

    /// The output under a point.
    pub fn output_at(&self, pos: Point<f64, Logical>) -> Option<smithay::output::Output> {
        self.space.outputs().find(|o| self.space.output_geometry(o).is_some_and(|g| g.to_f64().contains(pos))).cloned()
    }

    /// The output under the pointer: where new windows open.
    pub fn pointer_output(&self) -> Option<smithay::output::Output> {
        let at = self.seat.get_pointer()?.current_location();
        self.output_at(at).or_else(|| self.space.outputs().next().cloned())
    }

    /// The output a window is on: its tiles' output if it's tiled (even if
    /// the app draws past its tile), the one it fills if maximized or
    /// fullscreen, else the one it overlaps most, else (it hasn't drawn yet)
    /// the one its corner is on, else the first.
    pub fn output_of(&self, w: &Window) -> Option<smithay::output::Output> {
        // A tiled window is on its tiles' output.
        if let Some(s) = self.tile_set_of(w) {
            return Some(s.output.clone());
        }
        if let Some((_, o)) = self.filling.iter().find(|(f, o)| f == w && self.space.output_geometry(o).is_some()) {
            return Some(o.clone());
        }
        let geo = self.space.element_geometry(w);
        let area = |o: &smithay::output::Output| {
            geo.zip(self.space.output_geometry(o))
                .and_then(|(g, og)| g.intersection(og))
                .map(|r| r.size.w as i64 * r.size.h as i64)
                .unwrap_or(0)
        };
        self.space
            .outputs()
            .max_by_key(|o| area(o))
            .filter(|o| area(o) > 0)
            .or_else(|| {
                let at = geo?.loc;
                self.space.outputs().find(|o| self.space.output_geometry(o).is_some_and(|g| g.contains(at)))
            })
            .or_else(|| self.space.outputs().next())
            .cloned()
    }

    pub fn keyboard_focus(&self) -> Option<WlSurface> {
        self.seat.get_keyboard().and_then(|k| k.current_focus())
    }

    pub fn window_for_surface(&self, surface: &WlSurface) -> Option<Window> {
        self.space
            .elements()
            .chain(self.hidden_at.iter().map(|(w, _)| w))
            .find(|w| w.surface().as_ref() == Some(surface))
            .cloned()
    }

    /// Gives keyboard focus to a window (or none) and updates history.
    pub fn focus_window(&mut self, window: Option<Window>) {
        if self.is_locked() {
            return; // the lock surface keeps the keyboard
        }
        let serial = SERIAL_COUNTER.next_serial();
        let keyboard = self.seat.get_keyboard().expect("seat has a keyboard");
        self.scratchpad_focus_moves(window.as_ref());
        if let Some(w) = &window {
            // Focusing a minimised window (dock, ALT+TAB) brings it back.
            if let Some(i) = self.minimized.iter().position(|m| m == w) {
                self.minimized.swap_remove(i);
                if self.workspaces.workspace_of(w) == Some(self.workspaces.active()) {
                    self.show(w);
                    self.retile();
                }
            }
            // A group's hidden tab (ALT+TAB, the dock): show it.
            if self.tiling.groups.is_hidden(w) {
                self.show_tab(w);
            }
            self.space.raise_element(w, true);
            self.restack();
            self.workspaces.focus(w);
        }
        for w in self.space.elements() {
            w.set_activated(Some(w) == window.as_ref());
            w.configure();
        }
        let surface = window.as_ref().and_then(|w| w.surface());
        keyboard.set_focus(self, surface, serial);
        // An X11 window takes X's focus too.
        if let Some(w) = window.as_ref().filter(|w| crate::win::is_x11(w)) {
            self.x11_focus(w);
        }
        self.schedule_redraw();
        self.ipc_sync();
    }

    /// Gives a new window its stable IPC id.
    pub fn assign_window_id(&mut self, w: &Window) {
        if w.user_data().get::<WindowId>().is_none() {
            let id = self.next_window_id;
            w.user_data().insert_if_missing(|| WindowId(id));
            self.next_window_id += 1;
        }
    }

    pub fn window_id(&self, w: &Window) -> u64 {
        w.user_data().get::<WindowId>().map(|i| i.0).unwrap_or(0)
    }

    /// Focuses a window, switching to its workspace first if it's hidden.
    pub fn activate_window(&mut self, w: &Window) {
        if let Some(ws) = self.workspaces.workspace_of(w) {
            if ws != self.workspaces.active() {
                self.switch_workspace(ws);
            }
        }
        self.focus_window(Some(w.clone()));
    }

    /// The focused window: a scratchpad's if one has the keyboard, else
    /// the active workspace's.
    pub fn focused_window(&self) -> Option<Window> {
        self.scratchpads.focus.clone().or_else(|| self.workspaces.focused().cloned())
    }

    /// Switches workspace (0-based index).
    pub fn switch_workspace(&mut self, ws: usize) {
        let Some(switch) = self.workspaces.switch_to(ws) else { return };
        for w in &switch.hide {
            self.hide(w);
        }
        for w in &switch.show {
            // Not minimised windows, nor the hidden tabs of a group.
            if !self.minimized.contains(w) && !self.tiling.groups.is_hidden(w) {
                self.show(w);
            }
        }
        self.retile();
        // A scratchpad you're in comes along, keeping the keyboard.
        let focus = self.scratchpads.focus.clone().or_else(|| self.next_focus());
        self.focus_window(focus);
    }

    /// The window to focus when the focused one goes away: the most
    /// recently used on this workspace that's on screen (not minimised, not
    /// a group's hidden tab).
    pub fn next_focus(&self) -> Option<Window> {
        let ws = self.workspaces.active();
        self.workspaces
            .mru()
            .iter()
            .find(|w| {
                self.workspaces.workspace_of(w) == Some(ws)
                    && !self.minimized.contains(w)
                    && !self.tiling.groups.is_hidden(w)
            })
            .cloned()
    }

    /// Hides a window until it's focused again.
    pub fn minimize(&mut self, w: &Window) {
        if self.minimized.contains(w) || self.minimize_scratchpad(w) {
            return;
        }
        if self.fullscreen.contains(w) {
            self.unfullscreen(w);
        }
        // It comes back on its own, not as a tab.
        self.leave_group(w);
        self.minimized.push(w.clone());
        self.hide(w);
        self.retile();
        let next = self.next_focus();
        self.focus_window(next);
    }

    /// Moves the focused window to a workspace (0-based index).
    pub fn move_focused_to(&mut self, ws: usize) {
        let Some(w) = self.focused_window() else { return };
        if ws < self.workspaces.count() && self.workspaces.workspace_of(&w) != Some(ws) {
            self.leave_group(&w);
        }
        if self.workspaces.move_to(&w, ws) {
            self.end_snap(&w);
            self.hide(&w);
            self.retile();
            let next = self.workspaces.focused().cloned();
            self.focus_window(next);
        }
    }

    /// SUPER+ALT+arrow: the focused window to the workspace before (left,
    /// up) or after (right, down) this one, and there with it. Nothing at
    /// the first or last.
    pub fn move_focused_step(&mut self, dir: crate::layout::Dir) {
        use crate::layout::Dir;
        let Some(w) = self.focused_window() else { return };
        let ws = self.workspaces.active();
        let to = match dir {
            Dir::Left | Dir::Up => ws.checked_sub(1),
            Dir::Right | Dir::Down => Some(ws + 1).filter(|n| *n < self.workspaces.count()),
        };
        let Some(to) = to else { return };
        self.move_focused_to(to);
        if self.workspaces.workspace_of(&w) == Some(to) {
            self.switch_workspace(to);
            self.focus_window(Some(w));
        }
    }

    pub(crate) fn hide(&mut self, w: &Window) {
        if let Some(loc) = self.space.element_location(w) {
            self.hidden_at.push((w.clone(), loc));
        }
        self.space.unmap_elem(w);
    }

    pub(crate) fn show(&mut self, w: &Window) {
        let loc = self
            .hidden_at
            .iter()
            .position(|(h, _)| h == w)
            .map(|i| self.hidden_at.swap_remove(i).1)
            .unwrap_or_default();
        self.space.map_element(w.clone(), loc, false);
    }

    /// Forgets a closed window everywhere.
    pub fn forget_window(&mut self, w: &Window) {
        let id = self.window_id(w);
        if let Some((s, _)) = self.switcher.as_mut() {
            if !s.remove_window(id) {
                self.switcher = None;
            }
            self.switcher_sync();
        }
        // A shown scratchpad keeps the keyboard when another window closes.
        let keep = self.scratchpads.focus.clone().filter(|f| f != w);
        self.forget_scratchpad(w);
        self.forget_window_state(w);
        self.forget_tiled(w);
        self.forget_snap(w);
        self.workspaces.remove(w);
        self.hidden_at.retain(|(h, _)| h != w);
        self.minimized.retain(|m| m != w);
        self.space.unmap_elem(w);
        self.retile();
        let next = keep.or_else(|| self.next_focus());
        self.focus_window(next);
    }

    /// Environment for processes we start: where the compositor's IPC socket
    /// and this session's whiterosed are.
    pub fn ipc_env(&self) -> Vec<(&'static str, std::path::PathBuf)> {
        let mut env: Vec<_> = self.ipc.iter().map(|s| ("WHITEROSE_SOCKET", s.path.clone())).collect();
        env.push(("WHITEROSE_DAEMON_SOCKET", self.daemon_socket.clone()));
        if let Some((theme, size)) = &self.cursor_theme {
            env.push(("XCURSOR_THEME", theme.into()));
            env.push(("XCURSOR_SIZE", size.to_string().into()));
        }
        env
    }

    /// Starts `command` (a command line: shortcuts, scratchpads, `-c`) in a
    /// scope of its own, named after its program; its process id if it
    /// started.
    pub fn spawn(&self, command: &str) -> Option<u32> {
        let argv: Vec<String> = command.split_whitespace().map(String::from).collect();
        let id = crate::app_scope::id_for(argv.first()?);
        self.launch(&id, &argv, None)
    }

    /// Starts app `app_id` (`argv`, in `cwd`) in a scope of its own
    /// (app_scope.rs); its process id if it started.
    pub fn launch(&self, app_id: &str, argv: &[String], cwd: Option<&str>) -> Option<u32> {
        let protected = crate::app_scope::is_protected(app_id, &self.memory_protected, &self.keyboard.terminal);
        let mut cmd = crate::app_scope::command(app_id, argv, protected)?;
        cmd.env("WAYLAND_DISPLAY", &self.socket_name).envs(self.ipc_env());
        if let Some(dir) = cwd.filter(|d| std::path::Path::new(d).is_dir()) {
            cmd.current_dir(dir);
        }
        match cmd.spawn() {
            Ok(child) => {
                tracing::info!(app_id, ?argv, scoped = crate::app_scope::available(), protected, "spawned");
                Some(child.id())
            }
            Err(err) => {
                tracing::warn!(app_id, ?argv, %err, "failed to spawn");
                None
            }
        }
    }

    pub fn run_action(&mut self, action: Action) {
        // A scratchpad places itself: what would put it on a workspace, in
        // the tiles, a group, a zone or fullscreen leaves it be.
        if self.scratchpads.focus.is_some()
            && matches!(
                action,
                Action::MoveToWorkspace(_)
                    | Action::ToggleFloating
                    | Action::ToggleGroup
                    | Action::MakeMaster
                    | Action::ToggleSplit
                    | Action::ToggleFullscreen
                    | Action::SnapPicker(false)
            )
        {
            return;
        }
        match action {
            Action::Spawn(cmd) => {
                self.spawn(&cmd);
            }
            Action::Launch { app_id, argv, cwd } => {
                // (No id: named after its program, like a command line.)
                let id = if app_id.is_empty() {
                    crate::app_scope::id_for(argv.first().map_or("app", |s| s))
                } else {
                    app_id
                };
                self.launch(&id, &argv, cwd.as_deref());
            }
            Action::Scratchpad(id) => self.scratchpad_key(&id),
            Action::CloseWindow => {
                if let Some(w) = self.focused_window() {
                    w.close();
                }
            }
            Action::Workspace(n) => self.switch_workspace(usize::from(n).saturating_sub(1)),
            Action::MoveToWorkspace(n) => self.move_focused_to(usize::from(n).saturating_sub(1)),
            Action::MoveToWorkspaceStep(dir) => self.move_focused_step(dir),
            Action::MoveToOutput(dir) => self.move_focused_to_output(dir),
            Action::Quit => self.loop_signal.stop(),
            Action::Restart => {
                self.exit_code = crate::keybind::RESTART_CODE;
                self.loop_signal.stop();
            }
            Action::NextLayout => self.next_layout(),
            Action::SwitchVt(vt) => {
                if let Some(switch) = self.vt_switch.as_mut() {
                    switch(vt);
                }
            }
            Action::NightLight(n) => {
                if self.night_light != n {
                    tracing::info!(on = n.on, temperature = n.temperature, "night light");
                    self.night_light = n;
                    self.schedule_redraw();
                }
            }
            Action::ScreenshotScreen => self.request_shot(Default::default()),
            Action::Capture(shot) => self.request_shot(shot),
            Action::AppSwitcher => self.open_switcher(crate::switcher::Mode::Apps),
            Action::WindowSwitcher => self.open_switcher(crate::switcher::Mode::Windows),
            Action::FocusWindow(id) => {
                // Clicking a card in the switcher ends it.
                if self.switcher.take().is_some() {
                    self.switcher_sync();
                }
                let target = self.workspaces.mru().iter().find(|w| self.window_id(w) == id).cloned();
                match target {
                    Some(w) => self.activate_window(&w),
                    None => tracing::debug!(id, "focus request for unknown window"),
                }
            }
            Action::FocusApp(app_id) => {
                let focused = self.focused_window();
                let target = self.workspaces.next_for_app(
                    |w| crate::ipc::toplevel_ids(w).0.unwrap_or_default(),
                    &app_id,
                    focused.as_ref(),
                );
                if let Some(w) = target {
                    self.activate_window(&w);
                }
            }
            Action::DeleteScreenshot(path) => {
                self.delete_screenshot(std::path::Path::new(&path));
                self.screenshots_sync();
            }
            Action::Shell(req) => {
                use crate::keybind::ShellRequest as R;
                match req {
                    R::ClipboardList => self.clipboard_sync(),
                    R::ClipboardPaste(id) => self.clipboard_paste(id),
                    R::ClipboardPrepareDrag(id) => self.clipboard_prepare_drag(id),
                    R::ClipboardDelete(id) => self.clipboard_delete(id),
                    R::ClipboardClear => self.clipboard_clear(),
                    R::ScreenshotList => self.screenshots_sync(),
                    R::ScreenshotCopy(path) => self.screenshot_copy(std::path::Path::new(&path)),
                    R::WindowsList => {
                        let e = self.windows_event();
                        if let Some(ipc) = self.ipc.as_mut() {
                            ipc.update(e);
                        }
                    }
                }
            }
            Action::ToggleFullscreen => {
                if let Some(w) = self.focused_window() {
                    self.toggle_fullscreen(&w);
                }
            }
            Action::NextWindowLayout(output) => self.next_window_layout(output.as_deref()),
            Action::ToggleFloating => self.toggle_floating(),
            Action::FocusDirection(d) => self.focus_direction(d),
            Action::MoveDirection(d) => self.move_direction(d),
            Action::ResizeDirection(d) => self.resize_direction(d),
            Action::ResizeMode => self.tiling.key_resize = self.focused_window().is_some(),
            Action::ToggleSplit => self.toggle_split(),
            Action::MakeMaster => self.make_master(),
            Action::ToggleGroup => self.toggle_group(),
            Action::SnapPicker(under_pointer) => self.snap_picker(under_pointer),
            Action::NextTab => self.next_tab(true),
            Action::PreviousTab => self.next_tab(false),

            shell => {
                debug_assert!(shell.is_shell());
                let name = ipc::action_name(&shell).expect("every shell action has a wire name");
                match self.ipc.as_mut().filter(|s| s.client_count() > 0) {
                    Some(server) => server.update(ipc::Event::Action { name: name.into() }),
                    None => tracing::info!(name, "shell action, but no shell is connected"),
                }
            }
        }
    }
}

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

impl WhiteRose {
    /// `windows.appSwitcher.workspaces`: "current" or "all" (the default).
    pub fn switcher_setting(&mut self, key: &str, value: &serde_json::Value) {
        if key == "windows.appSwitcher.workspaces" {
            self.app_switcher_current_only = value.as_str() == Some("current");
        }
    }

    /// The windows a switcher offers, most recent first: ALT+TAB's only
    /// on this workspace if the setting says so.
    fn switcher_mru(&self, mode: crate::switcher::Mode) -> Vec<(u64, String)> {
        let here = self.workspaces.active();
        let only_here = mode == crate::switcher::Mode::Apps && self.app_switcher_current_only;
        self.workspaces
            .mru()
            .iter()
            .filter(|w| !only_here || self.workspaces.workspace_of(w) == Some(here))
            .map(|w| (self.window_id(w), crate::ipc::toplevel_ids(w).0.unwrap_or_default()))
            .collect()
    }

    pub fn open_switcher(&mut self, mode: crate::switcher::Mode) {
        if self.switcher.is_some() {
            return;
        }
        // Held modifiers keep it open; Shift only changes direction.
        let held = crate::keybind::Mods { shift: false, ..self.shortcut_mods };
        if let Some(s) = crate::switcher::Switcher::open(mode, &self.switcher_mru(mode)) {
            self.switcher = Some((s, held));
            self.switcher_sync();
        }
    }

    /// Focuses the selected window and closes the switcher.
    pub fn commit_switcher(&mut self) {
        let Some((s, _)) = self.switcher.take() else { return };
        self.switcher_sync();
        if let Some(id) = s.target() {
            if let Some(w) = self.workspaces.mru().iter().find(|w| self.window_id(w) == id).cloned() {
                self.activate_window(&w);
            }
        }
    }

    pub fn cancel_switcher(&mut self) {
        if self.switcher.take().is_some() {
            self.switcher_sync();
        }
    }

    pub fn switcher_sync(&mut self) {
        let event = crate::ipc::Event::Switcher { state: self.switcher.as_ref().map(|(s, _)| s.clone()) };
        if let Some(ipc) = self.ipc.as_mut() {
            ipc.update(event);
        }
    }
}
