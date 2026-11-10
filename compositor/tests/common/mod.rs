//! Shared test harness: a headless compositor with an optional fake
//! 1440x808 output (the Figma frame size) and a real Wayland client.
#![allow(dead_code)]

use std::{
    os::unix::net::UnixStream,
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

use smithay::{
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::{calloop::EventLoop, wayland_server::Display},
    utils::Transform,
};
use wayland_client::{
    delegate_noop,
    globals::{registry_queue_init, GlobalListContents},
    protocol::{
        wl_buffer::WlBuffer, wl_compositor::WlCompositor, wl_output::WlOutput, wl_registry, wl_shm,
        wl_shm_pool::WlShmPool, wl_surface::WlSurface,
    },
    Connection, Dispatch, EventQueue, QueueHandle,
};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{self, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{self, ZwlrLayerSurfaceV1},
};
use whiterose_wm::{keybind::Bindings, WhiteRose};

pub const W: i32 = 1440;
pub const H: i32 = 808;
pub const BAR: u32 = 26;

// ---------------------------------------------------------------- server side

pub fn runtime_dir() -> PathBuf {
    // Never touch the user's real clipboard history from tests.
    if std::env::var_os("WHITEROSE_CLIPBOARD_DIR").is_none() {
        std::env::set_var(
            "WHITEROSE_CLIPBOARD_DIR",
            std::env::temp_dir().join(format!("whiterose-clip-test-{}", std::process::id())),
        );
    }
    if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        return dir.into();
    }
    let dir = std::env::temp_dir().join(format!("whiterose-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::env::set_var("XDG_RUNTIME_DIR", &dir);
    dir
}

pub struct Headless {
    pub event_loop: EventLoop<'static, WhiteRose>,
    pub state: WhiteRose,
    pub output: Option<Output>,
}

impl Headless {
    pub fn new(with_output: bool) -> Self {
        runtime_dir();
        let mut event_loop = EventLoop::try_new().unwrap();
        let display: Display<WhiteRose> = Display::new().unwrap();
        let mut state = WhiteRose::new(&mut event_loop, display, Bindings::defaults("true"));
        // Each test compositor gets its own, empty clipboard history folder.
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        state.clipboard_dir = std::env::temp_dir().join(format!("whiterose-clip-{}-{n}", std::process::id()));
        state.clipboard = Default::default();
        let output = with_output.then(|| {
            let output = Output::new(
                "TEST-1".into(),
                PhysicalProperties {
                    size: (0, 0).into(),
                    subpixel: Subpixel::Unknown,
                    make: "T".into(),
                    model: "T".into(),
                },
            );
            let _ = output.create_global::<WhiteRose>(&state.display_handle);
            let mode = Mode { size: (W, H).into(), refresh: 60_000 };
            output.change_current_state(Some(mode), Some(Transform::Normal), None, Some((0, 0).into()));
            output.set_preferred(mode);
            state.space.map_output(&output, (0, 0));
            output
        });
        Headless { event_loop, state, output }
    }

    /// Another W x H output, at `at` in the global space (e.g. a monitor
    /// plugged in to the right of the first).
    pub fn add_output(&mut self, name: &str, at: (i32, i32)) -> Output {
        let output = Output::new(
            name.into(),
            PhysicalProperties {
                size: (0, 0).into(),
                subpixel: Subpixel::Unknown,
                make: "T".into(),
                model: "T".into(),
            },
        );
        let _ = output.create_global::<WhiteRose>(&self.state.display_handle);
        let mode = Mode { size: (W, H).into(), refresh: 60_000 };
        output.change_current_state(Some(mode), Some(Transform::Normal), None, Some(at.into()));
        output.set_preferred(mode);
        self.state.space.map_output(&output, at);
        output
    }

    /// One event-loop iteration, exactly as the real loop runs it.
    pub fn dispatch(&mut self) {
        self.event_loop.dispatch(Some(Duration::from_millis(2)), &mut self.state).unwrap();
        self.state.after_dispatch();
    }

    /// Runs `client` on a thread against this compositor. The client sends its
    /// findings, then stays connected until the server side has inspected state.
    pub fn run<T: Send + 'static>(
        &mut self,
        client: impl FnOnce(Connection) -> T + Send + 'static,
        inspect: impl FnOnce(&mut WhiteRose, T),
    ) {
        let socket = runtime_dir().join(&self.state.socket_name);
        let (result_tx, result_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let thread = std::thread::spawn(move || {
            let conn = Connection::from_socket(UnixStream::connect(socket).unwrap()).unwrap();
            result_tx.send(client(conn.clone())).unwrap();
            let _ = release_rx.recv_timeout(Duration::from_secs(5));
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        let result = loop {
            if let Ok(r) = result_rx.try_recv() {
                break r;
            }
            assert!(Instant::now() < deadline, "client did not finish");
            assert!(!thread.is_finished(), "client thread died");
            self.dispatch();
        };
        inspect(&mut self.state, result);
        release_tx.send(()).unwrap();
        thread.join().unwrap();
    }
}

// ---------------------------------------------------------------- client side

#[derive(Default)]
pub struct App {
    pub layer_configure: Option<(u32, u32)>,
    pub layer_closed: bool,
    pub toplevel_configured: bool,
    /// Every xdg_toplevel configure: (width, height, states).
    pub toplevel_configures: Vec<(i32, i32, Vec<u32>)>,
    /// wlr foreign-toplevel handles and what the compositor told us about each.
    pub handles: Vec<(ZwlrForeignToplevelHandleV1, HandleInfo)>,
    /// hyprland toplevel-export frame events, e.g. "buffer 400x300 stride 1600", "buffer_done", "failed".
    pub export_events: Vec<String>,
}

#[derive(Debug, Default, Clone)]
pub struct HandleInfo {
    pub title: String,
    pub app_id: String,
    pub states: Vec<u32>,
    pub done: bool,
    pub closed: bool,
}

/// Client bindings for hyprland-toplevel-export-v1 (generated from the same XML).
pub mod export {
    use wayland_client;
    use wayland_client::protocol::*;
    use wayland_protocols_wlr::foreign_toplevel::v1::client::*;
    pub mod __interfaces {
        use wayland_client::backend as wayland_backend;
        use wayland_client::protocol::__interfaces::*;
        use wayland_protocols_wlr::foreign_toplevel::v1::client::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocols/hyprland-toplevel-export-v1.xml");
    }
    use self::__interfaces::*;
    wayland_scanner::generate_client_code!("protocols/hyprland-toplevel-export-v1.xml");
}
pub use export::{hyprland_toplevel_export_frame_v1, hyprland_toplevel_export_manager_v1};
pub use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1::{self, ZwlrForeignToplevelHandleV1},
    zwlr_foreign_toplevel_manager_v1::{self, ZwlrForeignToplevelManagerV1},
};

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for App {
    fn event(
        app: &mut Self,
        _: &ZwlrForeignToplevelManagerV1,
        event: zwlr_foreign_toplevel_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_foreign_toplevel_manager_v1::Event::Toplevel { toplevel } = event {
            app.handles.push((toplevel, HandleInfo::default()));
        }
    }
    wayland_client::event_created_child!(App, ZwlrForeignToplevelManagerV1, [
        zwlr_foreign_toplevel_manager_v1::EVT_TOPLEVEL_OPCODE => (ZwlrForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for App {
    fn event(
        app: &mut Self,
        handle: &ZwlrForeignToplevelHandleV1,
        event: zwlr_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use zwlr_foreign_toplevel_handle_v1::Event;
        let Some((_, info)) = app.handles.iter_mut().find(|(h, _)| h == handle) else { return };
        match event {
            Event::Title { title } => info.title = title,
            Event::AppId { app_id } => info.app_id = app_id,
            Event::State { state } => {
                info.states = state.as_chunks::<4>().0.iter().map(|c| u32::from_ne_bytes(*c)).collect()
            }
            Event::Done => info.done = true,
            Event::Closed => info.closed = true,
            _ => {}
        }
    }
}

delegate_noop!(App: ignore hyprland_toplevel_export_manager_v1::HyprlandToplevelExportManagerV1);
delegate_noop!(App: ignore wayland_client::protocol::wl_seat::WlSeat);
delegate_noop!(App: ignore wayland_protocols::wp::idle_inhibit::zv1::client::zwp_idle_inhibit_manager_v1::ZwpIdleInhibitManagerV1);
delegate_noop!(App: ignore wayland_protocols::wp::idle_inhibit::zv1::client::zwp_idle_inhibitor_v1::ZwpIdleInhibitorV1);

impl Dispatch<hyprland_toplevel_export_frame_v1::HyprlandToplevelExportFrameV1, ()> for App {
    fn event(
        app: &mut Self,
        _: &hyprland_toplevel_export_frame_v1::HyprlandToplevelExportFrameV1,
        event: hyprland_toplevel_export_frame_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use hyprland_toplevel_export_frame_v1::Event;
        app.export_events.push(match event {
            Event::Buffer { format, width, height, stride } => {
                format!("buffer {width}x{height} stride {stride} format {:?}", format)
            }
            Event::BufferDone => "buffer_done".into(),
            Event::Ready { .. } => "ready".into(),
            Event::Failed => "failed".into(),
            Event::Flags { .. } => "flags".into(),
            _ => "other".into(),
        });
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for App {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
delegate_noop!(App: ignore WlCompositor);
delegate_noop!(App: ignore wayland_client::protocol::wl_region::WlRegion);
delegate_noop!(App: ignore WlSurface);
delegate_noop!(App: ignore WlOutput);
delegate_noop!(App: ignore wl_shm::WlShm);
delegate_noop!(App: ignore WlShmPool);
delegate_noop!(App: ignore WlBuffer);
delegate_noop!(App: ignore ZwlrLayerShellV1);
impl Dispatch<xdg_toplevel::XdgToplevel, ()> for App {
    fn event(
        app: &mut Self,
        _: &xdg_toplevel::XdgToplevel,
        event: xdg_toplevel::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_toplevel::Event::Configure { width, height, states } = event {
            let states = states.as_chunks::<4>().0.iter().map(|c| u32::from_ne_bytes(*c)).collect();
            app.toplevel_configures.push((width, height, states));
        }
    }
}

impl Dispatch<ZwlrLayerSurfaceV1, ()> for App {
    fn event(
        app: &mut Self,
        layer: &ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure { serial, width, height } => {
                layer.ack_configure(serial);
                app.layer_configure = Some((width, height));
            }
            zwlr_layer_surface_v1::Event::Closed => app.layer_closed = true,
            _ => {}
        }
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for App {
    fn event(
        _: &mut Self,
        base: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            base.pong(serial);
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, ()> for App {
    fn event(
        app: &mut Self,
        s: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            s.ack_configure(serial);
            app.toplevel_configured = true;
        }
    }
}

pub struct Client {
    pub queue: EventQueue<App>,
    pub app: App,
    pub compositor: WlCompositor,
    pub layer_shell: ZwlrLayerShellV1,
    pub xdg: xdg_wm_base::XdgWmBase,
    pub shm: wl_shm::WlShm,
}

impl Client {
    pub fn connect(conn: &Connection) -> Self {
        let (globals, queue) = registry_queue_init::<App>(conn).unwrap();
        let qh = queue.handle();
        Client {
            compositor: globals.bind(&qh, 4..=6, ()).expect("wl_compositor"),
            layer_shell: globals.bind(&qh, 1..=4, ()).expect("zwlr_layer_shell_v1"),
            xdg: globals.bind(&qh, 1..=6, ()).expect("xdg_wm_base"),
            shm: globals.bind(&qh, 1..=1, ()).expect("wl_shm"),
            queue,
            app: App::default(),
        }
    }

    pub fn until(&mut self, done: impl Fn(&App) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while !done(&self.app) {
            assert!(Instant::now() < deadline, "timed out waiting for the compositor");
            self.queue.blocking_dispatch(&mut self.app).unwrap();
        }
    }

    /// Like QuickShell's top bar: full width, 26px, reserves its space.
    pub fn open_bar(&mut self) -> (WlSurface, ZwlrLayerSurfaceV1) {
        let qh = self.queue.handle();
        let surface = self.compositor.create_surface(&qh, ());
        let layer = self.layer_shell.get_layer_surface(
            &surface,
            None,
            zwlr_layer_shell_v1::Layer::Top,
            "whiterose-bar".into(),
            &qh,
            (),
        );
        layer.set_size(0, BAR);
        layer.set_anchor(
            zwlr_layer_surface_v1::Anchor::Top
                | zwlr_layer_surface_v1::Anchor::Left
                | zwlr_layer_surface_v1::Anchor::Right,
        );
        layer.set_exclusive_zone(BAR as i32);
        surface.commit();
        self.until(|a| a.layer_configure.is_some() || a.layer_closed);
        (surface, layer)
    }
}

impl Client {
    /// Opens a plain window and waits for its first configure.
    pub fn open_window(&mut self) -> (WlSurface, xdg_toplevel::XdgToplevel) {
        let qh = self.queue.handle();
        let surface = self.compositor.create_surface(&qh, ());
        let xdg_surface = self.xdg.get_xdg_surface(&surface, &qh, ());
        let toplevel = xdg_surface.get_toplevel(&qh, ());
        surface.commit();
        self.until(|a| a.toplevel_configured);
        (surface, toplevel)
    }

    /// Waits for a toplevel configure beyond the `seen` already received.
    pub fn next_toplevel_configure(&mut self, seen: usize) -> (i32, i32, Vec<u32>) {
        self.until(|a| a.toplevel_configures.len() > seen);
        self.app.toplevel_configures[seen].clone()
    }
}

/// xdg_toplevel state values from the protocol.
pub const MAXIMIZED: u32 = 1;
pub const FULLSCREEN: u32 = 2;
pub const ACTIVATED: u32 = 4;

impl Client {
    /// Gives a surface real pixels (a w x h shared-memory buffer) and commits,
    /// so it has an input region and can be hit-tested like a real app.
    /// A w x h Argb8888 shared-memory buffer (read-write, as wl_shm requires).
    pub fn shm_buffer(&mut self, w: i32, h: i32) -> WlBuffer {
        use std::os::fd::AsFd;
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let qh = self.queue.handle();
        let stride = w * 4;
        let size = stride * h;
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("whiterose-shm-{}-{n}", std::process::id()));
        let file = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(true).open(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        file.set_len(size as u64).unwrap();
        let pool = self.shm.create_pool(file.as_fd(), size, &qh, ());
        let buffer = pool.create_buffer(0, w, h, stride, wl_shm::Format::Argb8888, &qh, ());
        pool.destroy();
        buffer
    }

    pub fn attach_buffer(&mut self, surface: &WlSurface, w: i32, h: i32) {
        let buffer = self.shm_buffer(w, h);
        surface.attach(Some(&buffer), 0, 0);
        surface.damage_buffer(0, 0, w, h);
        surface.commit();
        self.queue.roundtrip(&mut self.app).unwrap();
    }
}

// ---------------------------------------------------------------- driving

type Job = Box<dyn FnOnce(&mut WhiteRose) -> Box<dyn std::any::Any + Send> + Send>;

/// Lets a test client run code on the compositor (inject input, simulate a
/// presented frame, read state) at exact points in its own sequence.
pub struct Ask {
    jobs: mpsc::Sender<Job>,
    results: mpsc::Receiver<Box<dyn std::any::Any + Send>>,
}

impl Ask {
    pub fn run<R: Send + 'static>(&self, f: impl FnOnce(&mut WhiteRose) -> R + Send + 'static) -> R {
        self.jobs.send(Box::new(move |s| Box::new(f(s)))).unwrap();
        *self.results.recv_timeout(Duration::from_secs(5)).expect("compositor answered").downcast().unwrap()
    }
}

impl Headless {
    /// Runs `client` on a thread while this compositor dispatches, executing
    /// whatever the client asks for in between.
    pub fn drive<T: Send + 'static>(&mut self, client: impl FnOnce(Connection, Ask) -> T + Send + 'static) -> T {
        let socket = runtime_dir().join(&self.state.socket_name);
        let (job_tx, job_rx) = mpsc::channel::<Job>();
        let (res_tx, res_rx) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            let conn = Connection::from_socket(UnixStream::connect(socket).unwrap()).unwrap();
            client(conn, Ask { jobs: job_tx, results: res_rx })
        });
        let deadline = Instant::now() + Duration::from_secs(10);
        while !thread.is_finished() {
            assert!(Instant::now() < deadline, "client did not finish");
            if let Ok(job) = job_rx.try_recv() {
                let r = job(&mut self.state);
                self.state.after_dispatch();
                let _ = res_tx.send(r);
            }
            self.dispatch();
        }
        thread.join().unwrap()
    }
}
