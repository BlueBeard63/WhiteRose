//! Drag-and-drop out of a shell panel into an app window, the way the island
//! hands a clipboard entry or screenshot to Thunderbird or a browser: a real
//! client presses on a layer surface, starts a drag with a file list and an
//! icon, the pointer moves onto a window, and the window receives the drop.

mod common;

use std::{
    io::{Read, Write},
    os::fd::OwnedFd,
    sync::mpsc,
    time::{Duration, Instant},
};

use common::*;
use smithay::backend::input::ButtonState;
use wayland_client::{
    protocol::{
        wl_data_device::{self, WlDataDevice},
        wl_data_device_manager::{DndAction, WlDataDeviceManager},
        wl_data_offer::{self, WlDataOffer},
        wl_data_source::{self, WlDataSource},
        wl_pointer::{self, WlPointer},
        wl_seat::{self, WlSeat},
        wl_surface::WlSurface,
    },
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

const BTN_LEFT: u32 = 0x110;
const URIS: &str = "file:///home/me/Pictures/Screenshots/Screenshot%202026-11-09%2014-32-07.png\r\n";

/// The seat side of the test client: pointer and drag-and-drop events.
#[derive(Default)]
struct Dnd {
    pointer_on: Option<WlSurface>,
    button_serial: Option<u32>,
    /// Mime types announced for each offer the compositor created.
    offers: Vec<(WlDataOffer, Vec<String>)>,
    entered: Option<(WlSurface, Option<WlDataOffer>)>,
    dropped: bool,
    /// The drop target asked the source for this mime type.
    sent: Vec<String>,
    finished: bool,
}

impl Dispatch<wayland_client::protocol::wl_registry::WlRegistry, wayland_client::globals::GlobalListContents> for Dnd {
    fn event(
        _: &mut Self,
        _: &wayland_client::protocol::wl_registry::WlRegistry,
        _: wayland_client::protocol::wl_registry::Event,
        _: &wayland_client::globals::GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<WlSeat, ()> for Dnd {
    fn event(_: &mut Self, _: &WlSeat, _: wl_seat::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlDataDeviceManager, ()> for Dnd {
    fn event(
        _: &mut Self,
        _: &WlDataDeviceManager,
        _: <WlDataDeviceManager as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<WlPointer, ()> for Dnd {
    fn event(d: &mut Self, _: &WlPointer, event: wl_pointer::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match event {
            wl_pointer::Event::Enter { surface, .. } => d.pointer_on = Some(surface),
            wl_pointer::Event::Leave { .. } => d.pointer_on = None,
            wl_pointer::Event::Button { serial, .. } => d.button_serial = Some(serial),
            _ => {}
        }
    }
}
impl Dispatch<WlDataDevice, ()> for Dnd {
    fn event(
        d: &mut Self,
        _: &WlDataDevice,
        event: wl_data_device::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_device::Event::DataOffer { id } => d.offers.push((id, Vec::new())),
            wl_data_device::Event::Enter { surface, id, .. } => d.entered = Some((surface, id)),
            wl_data_device::Event::Leave => d.entered = None,
            wl_data_device::Event::Drop => d.dropped = true,
            _ => {}
        }
    }
    wayland_client::event_created_child!(Dnd, WlDataDevice, [
        wl_data_device::EVT_DATA_OFFER_OPCODE => (WlDataOffer, ()),
    ]);
}
impl Dispatch<WlDataOffer, ()> for Dnd {
    fn event(
        d: &mut Self,
        offer: &WlDataOffer,
        event: wl_data_offer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_data_offer::Event::Offer { mime_type } = event {
            if let Some((_, mimes)) = d.offers.iter_mut().find(|(o, _)| o == offer) {
                mimes.push(mime_type);
            }
        }
    }
}
impl Dispatch<WlDataSource, ()> for Dnd {
    fn event(
        d: &mut Self,
        _: &WlDataSource,
        event: wl_data_source::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_source::Event::Send { mime_type, fd } => {
                let payload = if mime_type == "text/uri-list" { URIS } else { "/home/me/x.png" };
                let _ = std::fs::File::from(fd).write_all(payload.as_bytes());
                d.sent.push(mime_type);
            }
            wl_data_source::Event::DndFinished => d.finished = true,
            _ => {}
        }
    }
}

/// What the client asks the (server-side) test driver to do.
enum Cmd {
    Move(f64, f64),
    Button(ButtonState),
    /// Centre of the first mapped window.
    WindowCentre,
    /// Whether the compositor is drawing a drag icon.
    HasIcon,
}

#[test]
fn panel_drags_a_file_into_a_window() {
    let mut wm = Headless::new(true);
    let socket = runtime_dir().join(&wm.state.socket_name);
    let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>();
    let (reply_tx, reply_rx) = mpsc::channel::<(f64, f64)>();

    let client = std::thread::spawn(move || {
        let ask = |c: Cmd| {
            cmd_tx.send(c).unwrap();
            reply_rx.recv_timeout(Duration::from_secs(3)).unwrap()
        };
        let conn = Connection::from_socket(std::os::unix::net::UnixStream::connect(socket).unwrap()).unwrap();
        let mut c = Client::connect(&conn);

        // A panel at the top centre (like the island) and an app window.
        let qh = c.queue.handle();
        let panel = c.compositor.create_surface(&qh, ());
        let layer = c.layer_shell.get_layer_surface(
            &panel,
            None,
            zwlr_layer_shell_v1::Layer::Top,
            "whiterose-bar".into(),
            &qh,
            (),
        );
        layer.set_size(400, 100);
        layer.set_anchor(zwlr_layer_surface_v1::Anchor::Top);
        panel.commit();
        c.until(|a| a.layer_configure.is_some());
        c.attach_buffer(&panel, 400, 100);
        let (window, _toplevel) = c.open_window();
        c.attach_buffer(&window, 800, 600);

        // The seat, on its own queue.
        let (globals, mut q) = wayland_client::globals::registry_queue_init::<Dnd>(&conn).unwrap();
        let dh = q.handle();
        let seat: WlSeat = globals.bind(&dh, 1..=7, ()).unwrap();
        let manager: WlDataDeviceManager = globals.bind(&dh, 3..=3, ()).unwrap();
        let _pointer = seat.get_pointer(&dh, ());
        let device = manager.get_data_device(&seat, &dh, ());
        let mut d = Dnd::default();
        let until = |q: &mut wayland_client::EventQueue<Dnd>, d: &mut Dnd, what: &str, done: &dyn Fn(&Dnd) -> bool| {
            let deadline = Instant::now() + Duration::from_secs(3);
            q.roundtrip(d).unwrap();
            while !done(d) {
                assert!(Instant::now() < deadline, "timed out waiting for {what}");
                q.blocking_dispatch(d).unwrap();
            }
        };

        // Press on the panel.
        q.roundtrip(&mut d).unwrap(); // the pointer exists before it moves
        ask(Cmd::Move(720.0, 50.0));
        until(&mut q, &mut d, "pointer on the panel", &|d| d.pointer_on.as_ref() == Some(&panel));
        ask(Cmd::Button(ButtonState::Pressed));
        until(&mut q, &mut d, "button press", &|d| d.button_serial.is_some());

        // Start dragging a file, with an icon.
        let source = manager.create_data_source(&dh, ());
        source.offer("text/uri-list".into());
        source.offer("text/plain".into());
        source.set_actions(DndAction::Copy);
        let icon = c.compositor.create_surface(&qh, ());
        device.start_drag(Some(&source), &panel, Some(&icon), d.button_serial.unwrap());
        c.attach_buffer(&icon, 32, 32);
        q.roundtrip(&mut d).unwrap();
        let has_icon = ask(Cmd::HasIcon).0 == 1.0;

        // Over the window: it's offered the file list.
        let (x, y) = ask(Cmd::WindowCentre);
        ask(Cmd::Move(x, y + 100.0));
        until(&mut q, &mut d, "drag entering the window", &|d| d.entered.as_ref().is_some_and(|(s, _)| *s == window));
        let offer = d.entered.as_ref().unwrap().1.clone().expect("an offer");
        let mimes = d.offers.iter().find(|(o, _)| *o == offer).unwrap().1.clone();
        offer.accept(0, Some("text/uri-list".into()));
        offer.set_actions(DndAction::Copy, DndAction::Copy);
        q.roundtrip(&mut d).unwrap();

        // Drop, and read what was dropped.
        ask(Cmd::Button(ButtonState::Released));
        until(&mut q, &mut d, "the drop", &|d| d.dropped);
        let (mut read, write) = std::io::pipe().unwrap();
        offer.receive("text/uri-list".into(), std::os::fd::AsFd::as_fd(&write));
        drop(OwnedFd::from(write));
        until(&mut q, &mut d, "the source to send the data", &|d| !d.sent.is_empty());
        let mut got = String::new();
        read.read_to_string(&mut got).unwrap();
        offer.finish();
        until(&mut q, &mut d, "dnd_finished", &|d| d.finished);
        let icon_after = ask(Cmd::HasIcon).0 == 1.0;
        (has_icon, mimes, got, icon_after)
    });

    // Drive the compositor and answer the client's requests until it's done.
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut time = 0;
    while !client.is_finished() {
        assert!(Instant::now() < deadline, "drag test did not finish");
        if let Ok(cmd) = cmd_rx.try_recv() {
            time += 10;
            let reply = match cmd {
                Cmd::Move(x, y) => {
                    wm.state.on_pointer_motion((x, y).into(), time);
                    (0.0, 0.0)
                }
                Cmd::Button(s) => {
                    wm.state.on_pointer_button(BTN_LEFT, s, time);
                    (0.0, 0.0)
                }
                Cmd::WindowCentre => {
                    let w = wm.state.space.elements().next().expect("a window");
                    let g = wm.state.space.element_geometry(w).unwrap();
                    ((g.loc.x + g.size.w / 2) as f64, (g.loc.y + g.size.h / 2) as f64)
                }
                Cmd::HasIcon => (if wm.state.dnd_icon.is_some() { 1.0 } else { 0.0 }, 0.0),
            };
            reply_tx.send(reply).unwrap();
        }
        // Pointer input comes from the test, not a client request: it only
        // reaches the client if the loop flushes after every iteration.
        wm.dispatch();
    }
    let (has_icon, mimes, got, icon_after) = client.join().unwrap();

    assert!(has_icon, "the drag icon is drawn under the pointer while dragging");
    assert_eq!(mimes, ["text/uri-list", "text/plain"], "the window is offered what the panel dragged");
    assert_eq!(got, URIS, "the window received the file list");
    assert!(!icon_after, "the icon goes away after the drop");
}

/// Regression: input that doesn't come from a client request (the host's
/// mouse, libinput) must reach an idle client at once. It used to sit in the
/// send buffer until some client happened to send a request, so clicks on the
/// island registered seconds late.
#[test]
fn input_reaches_an_idle_client_without_it_asking() {
    let mut wm = Headless::new(true);
    let socket = runtime_dir().join(&wm.state.socket_name);
    let (ready_tx, ready_rx) = mpsc::channel::<()>();
    let client = std::thread::spawn(move || {
        let conn = Connection::from_socket(std::os::unix::net::UnixStream::connect(socket).unwrap()).unwrap();
        let mut c = Client::connect(&conn);
        let qh = c.queue.handle();
        let panel = c.compositor.create_surface(&qh, ());
        let layer = c.layer_shell.get_layer_surface(
            &panel,
            None,
            zwlr_layer_shell_v1::Layer::Top,
            "whiterose-bar".into(),
            &qh,
            (),
        );
        layer.set_size(400, 100);
        layer.set_anchor(zwlr_layer_surface_v1::Anchor::Top);
        panel.commit();
        c.until(|a| a.layer_configure.is_some());
        c.attach_buffer(&panel, 400, 100);
        let (globals, mut q) = wayland_client::globals::registry_queue_init::<Dnd>(&conn).unwrap();
        let dh = q.handle();
        let seat: WlSeat = globals.bind(&dh, 1..=7, ()).unwrap();
        let _pointer = seat.get_pointer(&dh, ());
        let mut d = Dnd::default();
        q.roundtrip(&mut d).unwrap();
        // From here on the client only listens, like a shell waiting for a click.
        ready_tx.send(()).unwrap();
        while d.pointer_on.is_none() {
            q.blocking_dispatch(&mut d).unwrap();
        }
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while ready_rx.try_recv().is_err() {
        assert!(Instant::now() < deadline, "client set up");
        wm.dispatch();
    }
    // Let anything the setup caused settle, then move the pointer onto the panel.
    for _ in 0..5 {
        wm.dispatch();
    }
    wm.state.on_pointer_motion((640.0, 50.0).into(), 1);
    let deadline = Instant::now() + Duration::from_secs(2);
    while !client.is_finished() {
        assert!(Instant::now() < deadline, "the pointer enter never reached the idle client");
        wm.dispatch();
    }
    client.join().unwrap();
}
