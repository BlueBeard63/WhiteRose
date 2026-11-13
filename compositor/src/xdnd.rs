//! Drag and drop between Wayland and X11 apps (docs/plans/xwayland.md, step
//! 7). Smithay's X11 window manager has no XDND, so the compositor speaks it
//! itself, as its own small X11 client of our Xwayland (as KWin does).
//!
//! A Wayland drag over an X11 window: the drag grab hands it to the
//! compositor (`ClientDndGrabHandler::dnd_foreign`, a WhiteRose patch), which
//! is the XDND source: it owns XdndSelection, sends XdndEnter / Position /
//! Leave / Drop to the window, and when the X11 app asks for the data, has
//! the Wayland app write it (`wl_data_source.send`) and hands it over. The
//! X11 app's XdndStatus and XdndFinished go back to the Wayland app as
//! `target` / `action` and `dnd_finished`.
//!
//! The X11 connection lives on two threads (one runs the main loop's
//! commands, one reads X's events): X may be waiting on the compositor, so
//! the main loop never waits on X.

use std::{
    collections::HashMap,
    io::Read,
    os::unix::net::UnixStream,
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};

use smithay::{
    reexports::{
        calloop::{
            self,
            timer::{TimeoutAction, Timer},
            RegistrationToken,
        },
        wayland_server::{
            protocol::{wl_data_device_manager::DndAction, wl_data_source::WlDataSource, wl_surface::WlSurface},
            Resource,
        },
    },
    utils::{Logical, Point},
    wayland::selection::data_device::with_source_metadata,
    xwayland::XWaylandClientData,
};
use x11rb::{
    connection::Connection as _,
    protocol::{
        xproto::{
            Atom, AtomEnum, ClientMessageEvent, ConnectionExt as _, CreateWindowAux, EventMask, PropMode,
            SelectionNotifyEvent, SelectionRequestEvent, WindowClass, SELECTION_NOTIFY_EVENT,
        },
        Event,
    },
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
};

use crate::state::WhiteRose;

/// The XDND version we speak.
const VERSION: u32 = 5;
/// The most a drop hands over (an image, a long file list).
const MAX_DATA: usize = 16 << 20;
/// How long an X11 app gets to finish a drop, and a Wayland app to write it.
const FINISH_TIMEOUT: Duration = Duration::from_secs(10);
const READ_TIMEOUT: Duration = Duration::from_secs(5);

x11rb::atom_manager! {
    Atoms: AtomsCookie {
        XdndAware,
        XdndSelection,
        XdndEnter,
        XdndPosition,
        XdndStatus,
        XdndLeave,
        XdndDrop,
        XdndFinished,
        XdndTypeList,
        XdndActionCopy,
        XdndActionMove,
        XdndActionAsk,
        TARGETS,
        UTF8_STRING,
        TEXT,
    }
}

/// What the main loop asks of the XDND worker.
enum Cmd {
    Enter {
        target: u32,
        mimes: Vec<String>,
    },
    Position {
        target: u32,
        x: i32,
        y: i32,
        action: DndAction,
    },
    Leave {
        target: u32,
    },
    Drop {
        target: u32,
    },
    /// Answers an X11 app's request for the data with what `data` holds
    /// (None: refused).
    Serve {
        request: SelectionRequestEvent,
        data: Option<UnixStream>,
    },
}

/// What the worker tells the main loop.
pub enum XEvent {
    /// Connected: its resource id base (the idle check leaves it out).
    Ready { base: u32 },
    /// The X11 window takes the drop (or doesn't), with this action.
    Status { target: u32, accept: bool, action: DndAction },
    /// The X11 window is done with the drop.
    Finished { target: u32, success: bool, action: DndAction },
    /// The X11 window asks for the data as `mime`.
    Request { request: SelectionRequestEvent, mime: String },
}

/// A Wayland drag over X11 windows.
#[derive(Debug)]
struct OutDrag {
    source: WlDataSource,
    mimes: Vec<String>,
    actions: DndAction,
    /// The X11 window it's over.
    target: Option<u32>,
    accepted: bool,
    action: DndAction,
    dropped: bool,
    timeout: Option<RegistrationToken>,
}

/// The compositor's XDND side, while Xwayland runs.
#[derive(Debug, Default)]
pub struct DndState {
    cmds: Option<mpsc::Sender<Cmd>>,
    /// Our X11 client's resource id base, once connected.
    pub base: Option<u32>,
    out: Option<OutDrag>,
}

impl DndState {
    fn send(&self, cmd: Cmd) {
        if let Some(tx) = &self.cmds {
            let _ = tx.send(cmd);
        }
    }
}

impl WhiteRose {
    /// Xwayland is running: connects our XDND client to display `n`.
    pub(crate) fn start_xdnd(&mut self, n: u32) {
        let (events, rx) = calloop::channel::channel::<XEvent>();
        let inserted = self.loop_handle.insert_source(rx, |event, _, state| {
            if let calloop::channel::Event::Msg(e) = event {
                state.xdnd_event(e);
            }
        });
        if let Err(err) = inserted {
            tracing::warn!(%err, "drag and drop with X11 apps won't work");
            return;
        }
        let (tx, cmds) = mpsc::channel();
        std::thread::spawn(move || worker(n, cmds, events));
        self.xwayland.dnd = DndState { cmds: Some(tx), base: None, out: None };
    }

    /// Xwayland stopped: a drag it was taking is cancelled.
    pub(crate) fn stop_xdnd(&mut self) {
        let dnd = std::mem::take(&mut self.xwayland.dnd);
        if let Some(out) = dnd.out {
            self.end_out_drag(out, false);
        }
    }

    /// Whether drags over `surface` go through XDND: it's an X11 window.
    pub(crate) fn dnd_is_x11(&self, surface: &WlSurface) -> bool {
        self.xwayland.dnd.cmds.is_some()
            && surface.client().is_some_and(|c| c.get_data::<XWaylandClientData>().is_some())
    }

    /// The X11 window drawn on `surface`.
    fn x11_window_on(&self, surface: &WlSurface) -> Option<u32> {
        self.space.elements().find_map(|w| {
            let x = w.x11_surface()?;
            (x.wl_surface().as_ref() == Some(surface)).then(|| x.window_id())
        })
    }

    /// A Wayland drag of `source` is over the X11 window on `surface`.
    pub(crate) fn xdnd_motion(&mut self, surface: &WlSurface, at: Point<f64, Logical>, source: &WlDataSource) {
        let target = self.x11_window_on(surface);
        let same_source = self.xwayland.dnd.out.as_ref().is_some_and(|o| o.source == *source && !o.dropped);
        if !same_source {
            let (mimes, actions) = with_source_metadata(source, |m| (m.mime_types.clone(), m.dnd_action))
                .unwrap_or((Vec::new(), DndAction::empty()));
            if let Some(old) = self.xwayland.dnd.out.take() {
                self.end_out_drag(old, false);
            }
            self.xwayland.dnd.out = Some(OutDrag {
                source: source.clone(),
                mimes,
                actions,
                target: None,
                accepted: false,
                action: DndAction::empty(),
                dropped: false,
                timeout: None,
            });
        }
        let dnd = &mut self.xwayland.dnd;
        let Some(out) = dnd.out.as_mut() else { return };
        if out.target != target {
            if let Some(old) = out.target.take() {
                dnd.cmds.as_ref().map(|tx| tx.send(Cmd::Leave { target: old }));
            }
            out.accepted = false;
            out.target = target;
            if let Some(t) = target {
                let _ = dnd.cmds.as_ref().map(|tx| tx.send(Cmd::Enter { target: t, mimes: out.mimes.clone() }));
            }
        }
        if let Some(t) = target {
            let action = preferred(out.actions);
            let (x, y) = (at.x.round() as i32, at.y.round() as i32);
            let _ = dnd.cmds.as_ref().map(|tx| tx.send(Cmd::Position { target: t, x, y, action }));
        }
    }

    /// The drag left the X11 windows.
    pub(crate) fn xdnd_leave(&mut self) {
        let dnd = &mut self.xwayland.dnd;
        let Some(out) = dnd.out.as_mut() else { return };
        if let Some(t) = out.target.take() {
            let _ = dnd.cmds.as_ref().map(|tx| tx.send(Cmd::Leave { target: t }));
        }
        out.accepted = false;
        if out.source.version() >= 3 {
            out.source.target(None);
        }
    }

    /// Dropped on an X11 window: whether it takes the drop (then the source
    /// hears `dnd_finished` when it's done, or `cancelled`).
    pub(crate) fn xdnd_drop(&mut self) -> bool {
        let Some(mut out) = self.xwayland.dnd.out.take() else { return false };
        let Some(target) = out.target.filter(|_| out.accepted) else {
            if let Some(t) = out.target {
                self.xwayland.dnd.send(Cmd::Leave { target: t });
            }
            return false;
        };
        self.xwayland.dnd.send(Cmd::Drop { target });
        out.dropped = true;
        let timer = Timer::from_duration(FINISH_TIMEOUT);
        out.timeout = self
            .loop_handle
            .insert_source(timer, |_, _, state| {
                if let Some(out) = state.xwayland.dnd.out.take() {
                    tracing::info!("an X11 app didn't finish a drop in time");
                    state.end_out_drag(out, false);
                }
                TimeoutAction::Drop
            })
            .ok();
        self.xwayland.dnd.out = Some(out);
        true
    }

    /// Tells the Wayland source how its drag ended.
    fn end_out_drag(&mut self, mut out: OutDrag, done: bool) {
        if let Some(t) = out.timeout.take() {
            self.loop_handle.remove(t);
        }
        if !out.source.is_alive() {
            return;
        }
        if done && out.dropped && out.source.version() >= 3 {
            out.source.dnd_finished();
        } else {
            out.source.cancelled();
        }
    }

    fn xdnd_event(&mut self, event: XEvent) {
        match event {
            XEvent::Ready { base } => self.xwayland.dnd.base = Some(base),
            XEvent::Status { target, accept, action } => {
                let Some(out) = self.xwayland.dnd.out.as_mut() else { return };
                if out.target != Some(target) || out.dropped {
                    return;
                }
                let action = action & out.actions;
                let accept = accept && !action.is_empty();
                if accept != out.accepted && out.source.version() >= 3 {
                    out.source.target(accept.then(|| out.mimes.first().cloned()).flatten());
                }
                if accept && action != out.action && out.source.version() >= 3 {
                    out.source.action(action);
                }
                out.accepted = accept;
                out.action = action;
            }
            XEvent::Finished { target, success, .. } => {
                let matches = self.xwayland.dnd.out.as_ref().is_some_and(|o| o.dropped && o.target == Some(target));
                if matches {
                    let out = self.xwayland.dnd.out.take().unwrap();
                    self.end_out_drag(out, success);
                }
            }
            XEvent::Request { request, mime } => {
                let source = self.xwayland.dnd.out.as_ref().map(|o| (o.source.clone(), o.mimes.clone()));
                let data = source.and_then(|(source, mimes)| {
                    let mime = wayland_mime(&mime, &mimes)?;
                    let (read, write) = UnixStream::pair().ok()?;
                    read.set_read_timeout(Some(READ_TIMEOUT)).ok()?;
                    source.send(mime, std::os::fd::AsFd::as_fd(&write));
                    Some(read)
                });
                // The source only writes once it has the request.
                let _ = self.display_handle.flush_clients();
                self.xwayland.dnd.send(Cmd::Serve { request, data });
            }
        }
    }
}

/// The action to offer: copy if the source allows it, else move, else ask.
fn preferred(actions: DndAction) -> DndAction {
    [DndAction::Copy, DndAction::Move, DndAction::Ask]
        .into_iter()
        .find(|a| actions.contains(*a))
        .unwrap_or(DndAction::Copy)
}

/// The Wayland mime type for what an X11 app asked for: the same name, or
/// text for X's own text targets.
fn wayland_mime(asked: &str, offered: &[String]) -> Option<String> {
    if offered.iter().any(|m| m == asked) {
        return Some(asked.to_string());
    }
    let text = ["text/plain;charset=utf-8", "UTF8_STRING", "text/plain", "TEXT", "STRING"];
    if text.contains(&asked) {
        return text.iter().find(|t| offered.iter().any(|m| m == *t)).map(|t| t.to_string());
    }
    None
}

// ---------------------------------------------------------------- the X11 side

fn worker(n: u32, cmds: mpsc::Receiver<Cmd>, events: calloop::channel::Sender<XEvent>) {
    let Ok((conn, screen)) = x11rb::connect(Some(&format!(":{n}"))) else { return };
    let conn = Arc::new(conn);
    let root = conn.setup().roots[screen].root;
    let base = conn.setup().resource_id_base;
    let Some(atoms) = Atoms::new(&*conn).ok().and_then(|c| c.reply().ok()) else { return };
    let Ok(window) = conn.generate_id() else { return };
    let aux = CreateWindowAux::new();
    if conn.create_window(0, window, root, -10, -10, 1, 1, 0, WindowClass::INPUT_ONLY, 0, &aux).is_err() {
        return;
    }
    let _ = conn.flush();
    let _ = events.send(XEvent::Ready { base });
    // What's being dragged, for the TARGETS question.
    let offered = Arc::new(Mutex::new(Vec::<String>::new()));
    {
        let (conn, events, offered) = (conn.clone(), events.clone(), offered.clone());
        std::thread::spawn(move || read_events(&conn, atoms, window, &offered, &events));
    }
    let mut x = XSide { conn, atoms, window, aware: HashMap::new(), offered };
    while let Ok(cmd) = cmds.recv() {
        if x.run(cmd).is_err() {
            return;
        }
    }
}

struct XSide {
    conn: Arc<RustConnection>,
    atoms: Atoms,
    window: u32,
    /// Each window's XDND version (None: it doesn't take drops).
    aware: HashMap<u32, Option<u32>>,
    offered: Arc<Mutex<Vec<String>>>,
}

type XResult = Result<(), Box<dyn std::error::Error>>;

impl XSide {
    fn run(&mut self, cmd: Cmd) -> XResult {
        match cmd {
            Cmd::Enter { target, mimes } => {
                let reply =
                    self.conn.get_property(false, target, self.atoms.XdndAware, AtomEnum::ATOM, 0, 1)?.reply()?;
                let version = reply.value32().and_then(|mut v| v.next()).map(|v| v.min(VERSION));
                self.aware.insert(target, version);
                let Some(version) = version else { return Ok(()) };
                let types: Vec<Atom> = mimes.iter().map(|m| self.atom(m)).collect::<Result<_, _>>()?;
                *self.offered.lock().unwrap() = mimes;
                self.conn.set_selection_owner(self.window, self.atoms.XdndSelection, x11rb::CURRENT_TIME)?;
                let more = types.len() > 3;
                if more {
                    self.conn.change_property32(
                        PropMode::REPLACE,
                        self.window,
                        self.atoms.XdndTypeList,
                        AtomEnum::ATOM,
                        &types,
                    )?;
                }
                let t = |i: usize| types.get(i).copied().unwrap_or(0);
                self.message(
                    target,
                    self.atoms.XdndEnter,
                    [self.window, (version << 24) | u32::from(more), t(0), t(1), t(2)],
                )
            }
            Cmd::Position { target, x, y, action } => {
                if !self.takes_drops(target) {
                    return Ok(());
                }
                let at = ((x.clamp(0, 0x7fff) as u32) << 16) | (y.clamp(0, 0x7fff) as u32);
                let action = self.action_atom(action);
                self.message(target, self.atoms.XdndPosition, [self.window, 0, at, x11rb::CURRENT_TIME, action])
            }
            Cmd::Leave { target } => {
                if !self.takes_drops(target) {
                    return Ok(());
                }
                self.message(target, self.atoms.XdndLeave, [self.window, 0, 0, 0, 0])
            }
            Cmd::Drop { target } => {
                self.message(target, self.atoms.XdndDrop, [self.window, 0, x11rb::CURRENT_TIME, 0, 0])
            }
            Cmd::Serve { request, data } => {
                let bytes = data.and_then(|mut d| {
                    let mut buf = Vec::new();
                    let n = (&mut d).take(MAX_DATA as u64 + 1).read_to_end(&mut buf).ok()?;
                    (n <= MAX_DATA).then_some(buf)
                });
                answer(&self.conn, &request, bytes.map(|b| (request.target, 8, b)))
            }
        }
    }

    fn takes_drops(&self, target: u32) -> bool {
        self.aware.get(&target).copied().flatten().is_some()
    }

    fn atom(&self, name: &str) -> Result<Atom, Box<dyn std::error::Error>> {
        Ok(self.conn.intern_atom(false, name.as_bytes())?.reply()?.atom)
    }

    fn action_atom(&self, action: DndAction) -> Atom {
        if action.contains(DndAction::Move) {
            self.atoms.XdndActionMove
        } else if action.contains(DndAction::Ask) {
            self.atoms.XdndActionAsk
        } else {
            self.atoms.XdndActionCopy
        }
    }

    fn message(&self, to: u32, kind: Atom, data: [u32; 5]) -> XResult {
        let event = ClientMessageEvent::new(32, to, kind, data);
        self.conn.send_event(false, to, EventMask::NO_EVENT, event)?;
        self.conn.flush()?;
        Ok(())
    }
}

/// Writes `data` (target, format, bytes) where the request wants it and
/// tells the requestor; None refuses.
fn answer(conn: &RustConnection, r: &SelectionRequestEvent, data: Option<(Atom, u8, Vec<u8>)>) -> XResult {
    // (Old clients name no property: the target is the property.)
    let property = if r.property == x11rb::NONE { r.target } else { r.property };
    let property = match data {
        Some((kind, format, bytes)) => {
            conn.change_property(
                PropMode::REPLACE,
                r.requestor,
                property,
                kind,
                format,
                bytes.len() as u32 / (u32::from(format) / 8),
                &bytes,
            )?;
            property
        }
        None => x11rb::NONE,
    };
    let notify = SelectionNotifyEvent {
        response_type: SELECTION_NOTIFY_EVENT,
        sequence: 0,
        time: r.time,
        requestor: r.requestor,
        selection: r.selection,
        target: r.target,
        property,
    };
    conn.send_event(false, r.requestor, EventMask::NO_EVENT, notify)?;
    conn.flush()?;
    Ok(())
}

fn action_of(atoms: &Atoms, atom: Atom) -> DndAction {
    match atom {
        a if a == atoms.XdndActionMove => DndAction::Move,
        a if a == atoms.XdndActionAsk => DndAction::Ask,
        a if a == atoms.XdndActionCopy => DndAction::Copy,
        _ => DndAction::empty(),
    }
}

fn read_events(
    conn: &RustConnection,
    atoms: Atoms,
    window: u32,
    offered: &Mutex<Vec<String>>,
    events: &calloop::channel::Sender<XEvent>,
) {
    while let Ok(event) = conn.wait_for_event() {
        let sent = match event {
            Event::ClientMessage(m) if m.window == window && m.format == 32 => {
                let d = m.data.as_data32();
                if m.type_ == atoms.XdndStatus {
                    events.send(XEvent::Status { target: d[0], accept: d[1] & 1 == 1, action: action_of(&atoms, d[4]) })
                } else if m.type_ == atoms.XdndFinished {
                    events.send(XEvent::Finished {
                        target: d[0],
                        success: d[1] & 1 == 1,
                        action: action_of(&atoms, d[2]),
                    })
                } else {
                    Ok(())
                }
            }
            Event::SelectionRequest(r) if r.selection == atoms.XdndSelection && r.owner == window => {
                if r.target == atoms.TARGETS {
                    let names = offered.lock().unwrap().clone();
                    let mut list = vec![atoms.TARGETS];
                    for name in names {
                        if let Ok(Ok(reply)) = conn.intern_atom(false, name.as_bytes()).map(|c| c.reply()) {
                            list.push(reply.atom);
                        }
                    }
                    let bytes: Vec<u8> = list.iter().flat_map(|a| a.to_ne_bytes()).collect();
                    let _ = answer(conn, &r, Some((AtomEnum::ATOM.into(), 32, bytes)));
                    Ok(())
                } else {
                    let mime = conn
                        .get_atom_name(r.target)
                        .ok()
                        .and_then(|c| c.reply().ok())
                        .map(|n| String::from_utf8_lossy(&n.name).into_owned());
                    match mime {
                        Some(mime) => events.send(XEvent::Request { request: r, mime }),
                        None => {
                            let _ = answer(conn, &r, None);
                            Ok(())
                        }
                    }
                }
            }
            _ => Ok(()),
        };
        if sent.is_err() {
            return;
        }
    }
}
