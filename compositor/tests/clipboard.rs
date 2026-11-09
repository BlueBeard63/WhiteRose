//! Clipboard history with a real client copying through wl_data_device.

mod common;

use common::*;
use wayland_client::{
    delegate_noop,
    protocol::{
        wl_data_device::{self, WlDataDevice},
        wl_data_device_manager::WlDataDeviceManager,
        wl_data_offer::WlDataOffer,
        wl_data_source::{self, WlDataSource},
        wl_seat::WlSeat,
    },
    Connection, Dispatch, QueueHandle,
};
use whiterose_wm::clipboard::Content;

/// What the copying app hands over when the compositor asks.
struct Payload(&'static [u8]);

impl Dispatch<WlDataSource, Payload> for App {
    fn event(
        _: &mut Self,
        _: &WlDataSource,
        event: wl_data_source::Event,
        data: &Payload,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_data_source::Event::Send { fd, .. } = event {
            use std::io::Write;
            let mut f = std::fs::File::from(fd);
            let _ = f.write_all(data.0);
        }
    }
}
impl Dispatch<WlDataDevice, ()> for App {
    fn event(_: &mut Self, _: &WlDataDevice, _: wl_data_device::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
    wayland_client::event_created_child!(App, WlDataDevice, [
        wl_data_device::EVT_DATA_OFFER_OPCODE => (WlDataOffer, ()),
    ]);
}
delegate_noop!(App: ignore WlDataDeviceManager);
delegate_noop!(App: ignore WlDataOffer);

fn copy(c: &mut Client, conn: &Connection, mimes: &[&str], data: &'static [u8]) {
    let (globals, _) = wayland_client::globals::registry_queue_init::<App>(conn).unwrap();
    let qh = c.queue.handle();
    let manager: WlDataDeviceManager = globals.bind(&qh, 1..=3, ()).unwrap();
    let seat: WlSeat = globals.bind(&qh, 1..=7, ()).unwrap();
    let device = manager.get_data_device(&seat, &qh, ());
    let source = manager.create_data_source(&qh, Payload(data));
    for m in mimes {
        source.offer(m.to_string());
    }
    device.set_selection(Some(&source), 0);
    // Let the compositor ask for the data, then let its worker read it.
    for _ in 0..5 {
        c.queue.roundtrip(&mut c.app).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(40));
    }
    std::mem::forget((device, source));
}

#[test]
fn copied_text_is_recorded_and_secrets_are_not() {
    let mut h = Headless::new(true);
    h.state.clipboard_clear();
    h.run(
        |conn| {
            let mut c = Client::connect(&conn);
            c.open_window(); // apps need keyboard focus to set the clipboard
            copy(&mut c, &conn, &["text/plain;charset=utf-8", "text/plain"], b"hello from a real client");
            copy(&mut c, &conn, &["text/plain", "x-kde-passwordManagerHint"], b"hunter2");
        },
        |state, ()| {
            let texts: Vec<&str> = state
                .clipboard
                .entries
                .iter()
                .filter_map(|e| match &e.content {
                    Content::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(texts, vec!["hello from a real client"], "the password-manager copy must not be stored");
            // Saved to disk.
            let saved = std::fs::read_to_string(state.clipboard_dir.join("history.json")).unwrap();
            assert!(saved.contains("hello from a real client") && !saved.contains("hunter2"));
        },
    );
}

#[test]
fn copied_files_are_stored_as_paths() {
    let mut h = Headless::new(true);
    h.state.clipboard_clear();
    h.run(
        |conn| {
            let mut c = Client::connect(&conn);
            c.open_window();
            copy(&mut c, &conn, &["x-special/gnome-copied-files", "text/uri-list"], b"file:///tmp/a%20b.txt\r\n");
        },
        |state, ()| {
            assert_eq!(
                state.clipboard.entries.first().map(|e| e.content.clone()),
                Some(Content::Files { paths: vec!["/tmp/a b.txt".into()] })
            );
        },
    );
}
