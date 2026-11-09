//! Compositor state. Structure follows Smithay's smallvil example (MIT).

use std::{ffi::OsString, process::Command, sync::Arc};

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
    pub socket_name: OsString,
    pub display_handle: DisplayHandle,
    pub loop_signal: LoopSignal,
    pub loop_handle: LoopHandle<'static, WhiteRose>,
    /// Shell IPC. None if the socket could not be created.
    pub ipc: Option<ipc::Server>,

    pub space: Space<Window>,
    pub workspaces: Workspaces<Window>,
    /// Where hidden windows were, so switching back restores them.
    pub hidden_at: Vec<(Window, Point<i32, Logical>)>,
    /// Floating geometry of maximized/fullscreen windows, for restoring.
    pub restore_geometry: Vec<(Window, smithay::utils::Rectangle<i32, Logical>)>,
    pub fullscreen: Option<Window>,
    /// Output and usable area last used to fit maximized/fullscreen windows.
    /// PrtSc pressed: capture on the next frame.
    pub screenshot_pending: bool,
    /// Worker threads report finished captures here.
    pub screenshot_tx: smithay::reexports::calloop::channel::Sender<crate::screenshot::Outcome>,
    /// Paths this compositor saved; the only files the shell may ask it to delete.
    pub saved_screenshots: Vec<std::path::PathBuf>,
    next_window_id: u64,
    /// The open switcher and the modifiers that keep it open (released = commit).
    pub switcher: Option<(crate::switcher::Switcher, crate::keybind::Mods)>,
    /// Modifiers of the shortcut that triggered the current action.
    pub shortcut_mods: crate::keybind::Mods,
    pub fitted_to: Option<(smithay::utils::Rectangle<i32, Logical>, smithay::utils::Rectangle<i32, Logical>)>,

    pub bindings: Bindings,
    /// Keys whose press was a shortcut; their release is swallowed too.
    pub swallowed: Vec<Keycode>,
    /// SUPER is down and nothing else has been pressed since.
    pub logo_tap_pending: bool,

    /// Wakes the backend to draw a frame. None when running headless (tests).
    pub redraw: Option<Ping>,
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
    /// Clipboard history (saved under clipboard_dir).
    pub clipboard: crate::clipboard::History,
    pub clipboard_dir: std::path::PathBuf,
    pub clipboard_tx: smithay::reexports::calloop::channel::Sender<crate::clipboard_capture::Captured>,
    pub popups: PopupManager,
    pub seat: Seat<Self>,
}

impl WhiteRose {
    pub fn new(event_loop: &mut EventLoop<'static, Self>, display: Display<Self>, bindings: Bindings) -> Self {
        let dh = display.handle();

        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let mut seat_state = SeatState::new();
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        let layer_shell_state = WlrLayerShellState::new::<Self>(&dh);
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
        let ipc = ipc::Server::bind(&ipc_path, &event_loop.handle())
            .map_err(|err| tracing::warn!(%err, "shell ipc disabled"))
            .ok();

        Self {
            start_time: std::time::Instant::now(),
            socket_name,
            display_handle: dh,
            loop_signal: event_loop.get_signal(),
            loop_handle: event_loop.handle(),
            ipc,
            space: Space::default(),
            workspaces: Workspaces::new(WORKSPACES),
            hidden_at: Vec::new(),
            restore_geometry: Vec::new(),
            fullscreen: None,
            fitted_to: None,
            next_window_id: 1,
            switcher: None,
            shortcut_mods: crate::keybind::Mods::NONE,
            screenshot_pending: false,
            screenshot_tx,
            saved_screenshots: Vec::new(),
            bindings,
            swallowed: Vec::new(),
            logo_tap_pending: false,
            redraw: None,
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
            clipboard,
            clipboard_dir,
            clipboard_tx,
            popups: PopupManager::default(),
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
        let output_hit = self
            .space
            .outputs()
            .find_map(|o| self.space.output_geometry(o).filter(|g| g.to_f64().contains(pos)).map(|g| (o, g)));

        let layer_hit = |layer: Layer| {
            let (output, geo) = output_hit?;
            let map = layer_map_for_output(output);
            let local = pos - geo.loc.to_f64();
            let surface = map.layer_under(layer, local)?;
            let layer_geo = map.layer_geometry(surface)?;
            surface
                .surface_under(local - layer_geo.loc.to_f64(), WindowSurfaceType::ALL)
                .map(|(s, p)| (s, (p + layer_geo.loc + geo.loc).to_f64()))
        };
        let window_hit = || {
            self.space.element_under(pos).and_then(|(window, location)| {
                window
                    .surface_under(pos - location.to_f64(), WindowSurfaceType::ALL)
                    .map(|(s, p)| (s, (p + location).to_f64()))
            })
        };

        if self.active_fullscreen().is_some() {
            return layer_hit(Layer::Overlay).or_else(window_hit);
        }
        layer_hit(Layer::Overlay)
            .or_else(|| layer_hit(Layer::Top))
            .or_else(window_hit)
            .or_else(|| layer_hit(Layer::Bottom))
            .or_else(|| layer_hit(Layer::Background))
    }

    /// The top or overlay layer surface (bar, panel, launcher) under `pos`.
    pub fn top_layer_at(&self, pos: Point<f64, Logical>) -> Option<LayerSurface> {
        let (output, geo) = self
            .space
            .outputs()
            .find_map(|o| self.space.output_geometry(o).filter(|g| g.to_f64().contains(pos)).map(|g| (o, g)))?;
        let map = layer_map_for_output(output);
        let local = pos - geo.loc.to_f64();
        [Layer::Overlay, Layer::Top].into_iter().find_map(|l| map.layer_under(l, local).cloned())
    }

    /// The part of the first output not reserved by panels (e.g. the bar).
    pub fn usable_area(&self) -> smithay::utils::Rectangle<i32, Logical> {
        self.space
            .outputs()
            .next()
            .map(|o| {
                let origin = self.space.output_geometry(o).map(|g| g.loc).unwrap_or_default();
                let zone = layer_map_for_output(o).non_exclusive_zone();
                smithay::utils::Rectangle::new(origin + zone.loc, zone.size)
            })
            .unwrap_or_default()
    }

    pub fn keyboard_focus(&self) -> Option<WlSurface> {
        self.seat.get_keyboard().and_then(|k| k.current_focus())
    }

    pub fn window_for_surface(&self, surface: &WlSurface) -> Option<Window> {
        self.space
            .elements()
            .chain(self.hidden_at.iter().map(|(w, _)| w))
            .find(|w| w.toplevel().is_some_and(|t| t.wl_surface() == surface))
            .cloned()
    }

    /// Gives keyboard focus to a window (or none) and updates history.
    pub fn focus_window(&mut self, window: Option<Window>) {
        let serial = SERIAL_COUNTER.next_serial();
        let keyboard = self.seat.get_keyboard().expect("seat has a keyboard");
        if let Some(w) = &window {
            self.space.raise_element(w, true);
            self.workspaces.focus(w);
        }
        for w in self.space.elements() {
            w.set_activated(Some(w) == window.as_ref());
            if let Some(t) = w.toplevel() {
                t.send_pending_configure();
            }
        }
        let surface = window.and_then(|w| w.toplevel().map(|t| t.wl_surface().clone()));
        keyboard.set_focus(self, surface, serial);
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

    pub fn focused_window(&self) -> Option<Window> {
        self.workspaces.focused().cloned()
    }

    /// Switches workspace (0-based index).
    pub fn switch_workspace(&mut self, ws: usize) {
        let Some(switch) = self.workspaces.switch_to(ws) else { return };
        for w in &switch.hide {
            self.hide(w);
        }
        for w in &switch.show {
            self.show(w);
        }
        let focus = self.workspaces.focused().cloned();
        self.focus_window(focus);
    }

    /// Moves the focused window to a workspace (0-based index).
    pub fn move_focused_to(&mut self, ws: usize) {
        let Some(w) = self.focused_window() else { return };
        if self.workspaces.move_to(&w, ws) {
            self.hide(&w);
            let next = self.workspaces.focused().cloned();
            self.focus_window(next);
        }
    }

    fn hide(&mut self, w: &Window) {
        if let Some(loc) = self.space.element_location(w) {
            self.hidden_at.push((w.clone(), loc));
        }
        self.space.unmap_elem(w);
    }

    fn show(&mut self, w: &Window) {
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
        self.forget_window_state(w);
        self.workspaces.remove(w);
        self.hidden_at.retain(|(h, _)| h != w);
        self.space.unmap_elem(w);
        let next = self.workspaces.focused().cloned();
        self.focus_window(next);
    }

    /// Environment for spawned processes so the shell can find the IPC socket.
    fn ipc_env(&self) -> Vec<(&'static str, std::path::PathBuf)> {
        self.ipc.iter().map(|s| ("WHITEROSE_SOCKET", s.path.clone())).collect()
    }

    pub fn spawn(&self, command: &str) {
        let mut parts = command.split_whitespace();
        let Some(program) = parts.next() else { return };
        match Command::new(program).args(parts).env("WAYLAND_DISPLAY", &self.socket_name).envs(self.ipc_env()).spawn() {
            Ok(_) => tracing::info!(command, "spawned"),
            Err(err) => tracing::warn!(command, %err, "failed to spawn"),
        }
    }

    pub fn run_action(&mut self, action: Action) {
        match action {
            Action::Spawn(cmd) => self.spawn(&cmd),
            Action::CloseWindow => {
                if let Some(t) = self.focused_window().and_then(|w| w.toplevel().cloned()) {
                    t.send_close();
                }
            }
            Action::Workspace(n) => self.switch_workspace(usize::from(n).saturating_sub(1)),
            Action::MoveToWorkspace(n) => self.move_focused_to(usize::from(n).saturating_sub(1)),
            Action::Quit => self.loop_signal.stop(),
            Action::ScreenshotScreen => self.request_screenshot(),
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
                }
            }
            Action::ToggleFullscreen => {
                if let Some(w) = self.focused_window() {
                    self.toggle_fullscreen(&w);
                }
            }
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
    fn switcher_mru(&self) -> Vec<(u64, String)> {
        self.workspaces
            .mru()
            .iter()
            .map(|w| (self.window_id(w), crate::ipc::toplevel_ids(w).0.unwrap_or_default()))
            .collect()
    }

    pub fn open_switcher(&mut self, mode: crate::switcher::Mode) {
        if self.switcher.is_some() {
            return;
        }
        // Held modifiers keep it open; Shift only changes direction.
        let held = crate::keybind::Mods { shift: false, ..self.shortcut_mods };
        if let Some(s) = crate::switcher::Switcher::open(mode, &self.switcher_mru()) {
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
