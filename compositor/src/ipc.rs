//! Compositor <-> shell IPC: a Unix socket speaking newline-delimited JSON.
//!
//! The shell (QuickShell) connects to `$WHITEROSE_SOCKET`, which every process
//! spawned by the compositor inherits. On connect it gets the current state;
//! after that it gets events as they happen. Nothing is polled.
//!
//! ```text
//! <- {"event":"workspaces","active":2,"count":9,"occupied":[1,2]}
//! <- {"event":"focus","app_id":"org.gnome.Ptyxis","title":"~"}
//! <- {"event":"action","name":"screenshot_region"}
//! -> {"cmd":"workspace","n":3}
//! -> {"cmd":"spawn","command":"ptyxis --standalone"}
//! ```

use crate::win::WinExt;
use std::{
    io::{ErrorKind, Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use smithay::reexports::calloop::{generic::Generic, Interest, LoopHandle, Mode, PostAction};

use crate::{
    keybind::{Action, ShellRequest},
    state::WhiteRose,
};

/// Messages from the compositor to the shell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// Workspace numbers are 1-based, as shown to the user. `layouts`: the
    /// window layout of the active workspace on each output, by output name
    /// ("floating", "dwindle", "master"), for each screen's bar.
    Workspaces {
        active: u8,
        count: u8,
        occupied: Vec<u8>,
        #[serde(default)]
        layouts: std::collections::BTreeMap<String, String>,
    },
    Focus {
        app_id: Option<String>,
        title: Option<String>,
    },
    /// A shortcut the shell implements (launcher, switchers, screenshot...).
    Action {
        name: String,
    },
    /// Every window, most recently used first (dock, switchers).
    Windows {
        windows: Vec<WindowInfo>,
    },
    /// ALT+TAB / SUPER+TAB state; `state` is null when closed.
    Switcher {
        state: Option<crate::switcher::Switcher>,
    },
    /// Clipboard history, newest first; copied files re-checked on disk.
    Clipboard {
        entries: Vec<crate::clipboard::EntryView>,
    },
    /// The newest screenshots in the Screenshots folder (absolute paths).
    Screenshots {
        files: Vec<String>,
    },
    /// A screenshot was saved (and put on the clipboard as image/png).
    Screenshot {
        path: String,
        width: u32,
        height: u32,
    },
    ScreenshotFailed {
        error: String,
    },
    /// The session is locked (only the lock screen is shown). Sent on
    /// connect too, so a restarted shell puts its lock screen back.
    Lock {
        locked: bool,
    },
    /// The outputs (by name, e.g. "DP-1") where a window sits right under
    /// the idle island (a maximized window's title bar): the shell tucks
    /// that screen's island into its bar. And those with a fullscreen
    /// window (the bar isn't drawn there): the island waits in the top edge
    /// and drops over the window when a view opens.
    Island {
        covered: Vec<String>,
        fullscreen: Vec<String>,
    },
    /// Every connected output and what this backend can change about them
    /// (the Displays page); `trial_until` while a layout is being tried out.
    Outputs {
        outputs: Vec<crate::outputs::OutputView>,
        capabilities: crate::outputs::Capabilities,
        trial_until: Option<u64>,
    },
    /// Every shortcut and its keys (Keyboard & shortcuts page).
    Shortcuts {
        shortcuts: Vec<crate::keyboard::ShortcutView>,
    },
    /// The keyboard layouts in use (the system's if none are set), with
    /// their names, and which one is active.
    Keyboard {
        layouts: Vec<crate::keyboard::LayoutView>,
        active: usize,
    },
    /// Every layer surface (the bar, panels, wallpaper), sent only to the
    /// client that asked with `{"debug_layers":true}` (UI tests).
    Layers {
        layers: Vec<LayerInfo>,
    },
    /// The snap layout picker; null when it's closed (snapping.rs).
    SnapPicker {
        picker: Option<PickerView>,
    },
}

/// The snap layout picker, for the shell to draw exactly there: `x`, `y`
/// on the output named; tiles and cells relative to the picker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PickerView {
    pub output: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    /// A card under the pointer (a WhiteRose app's maximize button), not
    /// hanging from the bar like the island.
    pub card: bool,
    /// The window's title.
    pub title: String,
    pub layouts: Vec<PickerLayoutView>,
    /// [layout, cell] under the pointer or chosen with the arrows.
    pub hovered: Option<[usize; 2]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PickerLayoutView {
    /// `halves`, `thirds`, `quarters`, `two-thirds-left`,
    /// `two-thirds-right`, `zones`.
    pub id: String,
    /// [x, y, w, h]
    pub rect: [i32; 4],
    pub cells: Vec<[i32; 4]>,
}

impl Event {
    pub fn to_line(&self) -> String {
        let mut s = serde_json::to_string(self).expect("events always serialize");
        s.push('\n');
        s
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowInfo {
    pub id: u64,
    pub app_id: Option<String>,
    pub title: Option<String>,
    /// 1-based.
    pub workspace: u8,
    pub focused: bool,
    /// [x, y, w, h] in global logical pixels while it's on screen.
    #[serde(default)]
    pub geometry: Option<[i32; 4]>,
}

/// A layer-shell surface on screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerInfo {
    /// What the client called it ("whiterose-bar").
    pub namespace: String,
    pub output: String,
    /// "background", "bottom", "top" or "overlay".
    pub layer: String,
    /// [x, y, w, h] in global logical pixels.
    pub geometry: [i32; 4],
}

/// Messages from the shell to the compositor.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    /// Switch to workspace `n` (1-based).
    Workspace {
        n: u8,
    },
    /// Move the focused window to workspace `n` (1-based).
    MoveToWorkspace {
        n: u8,
    },
    Spawn {
        command: String,
    },
    /// Start an app in its own scope (app_scope.rs): the launcher's and
    /// dock's desktop entries, and files opened from the shell.
    Launch {
        app_id: String,
        argv: Vec<String>,
        #[serde(default)]
        cwd: Option<String>,
    },
    CloseWindow,
    /// Open a switcher without held modifiers (mouse, dock, scripts): it
    /// stays open until Enter, Esc or a click.
    Switcher {
        mode: crate::switcher::Mode,
    },
    FocusWindow {
        id: u64,
    },
    FocusApp {
        app_id: String,
    },
    /// Re-send the clipboard history (re-checks copied files on disk).
    ClipboardList,
    /// Put a history entry back on the clipboard.
    ClipboardPaste {
        id: u64,
    },
    /// Make the named copy of an image entry before it's dragged out.
    ClipboardPrepareDrag {
        id: u64,
    },
    ClipboardDelete {
        id: u64,
    },
    ClipboardClear,
    ScreenshotList,
    /// Copy a screenshot from the tray onto the clipboard.
    ScreenshotCopy {
        path: String,
    },
    /// Capture the whole screen (e.g. the screenshot toolbar's "Screen" mode).
    Screenshot,
    /// The screenshot tool: `region` [x, y, w, h] (global logical) or the
    /// screen under the pointer; with the pointer drawn in; saved, or only
    /// copied.
    Capture {
        #[serde(default)]
        region: Option<[i32; 4]>,
        #[serde(default)]
        pointer: bool,
        #[serde(default = "yes")]
        save: bool,
    },
    /// Send the window list (with where each window is) now.
    WindowsList,
    /// Night light from the quick settings: warm the screen to `temperature` K.
    NightLight {
        on: bool,
        #[serde(default = "neutral")]
        temperature: u32,
    },
    /// End the session (Log out).
    Quit,
    /// Start the session again (whiterose-launch), e.g. to switch to an update.
    Restart,
    /// Show each output's number and name on it (Displays page).
    IdentifyOutputs,
    /// Delete a screenshot this compositor saved (the toast's trash button).
    DeleteScreenshot {
        path: String,
    },
    /// The next window layout (the bar's layout button), on the output
    /// named, else where you're working (as SUPER+T).
    NextWindowLayout {
        #[serde(default)]
        output: Option<String>,
    },
    /// Settings is recording a new shortcut: keys reach it instead of
    /// triggering shortcuts, until it says so or disconnects.
    PauseShortcuts {
        on: bool,
    },
    /// Open the snap layout picker for the focused window (as SUPER+Z), or
    /// for the window under the pointer, just below it (a WhiteRose app's
    /// maximize button).
    SnapPicker {
        #[serde(default)]
        under_pointer: bool,
    },
}

fn yes() -> bool {
    true
}

fn neutral() -> u32 {
    crate::night_light::NEUTRAL
}

impl Request {
    pub fn parse(line: &str) -> Result<Request, String> {
        serde_json::from_str(line).map_err(|e| format!("bad request {line:?}: {e}"))
    }

    pub fn into_action(self) -> Action {
        match self {
            Request::Workspace { n } => Action::Workspace(n),
            Request::MoveToWorkspace { n } => Action::MoveToWorkspace(n),
            Request::Spawn { command } => Action::Spawn(command),
            Request::Launch { app_id, argv, cwd } => Action::Launch { app_id, argv, cwd },
            Request::CloseWindow => Action::CloseWindow,
            Request::Screenshot => Action::ScreenshotScreen,
            Request::ClipboardList => Action::Shell(ShellRequest::ClipboardList),
            Request::ClipboardPaste { id } => Action::Shell(ShellRequest::ClipboardPaste(id)),
            Request::ClipboardPrepareDrag { id } => Action::Shell(ShellRequest::ClipboardPrepareDrag(id)),
            Request::ClipboardDelete { id } => Action::Shell(ShellRequest::ClipboardDelete(id)),
            Request::ClipboardClear => Action::Shell(ShellRequest::ClipboardClear),
            Request::ScreenshotList => Action::Shell(ShellRequest::ScreenshotList),
            Request::ScreenshotCopy { path } => Action::Shell(ShellRequest::ScreenshotCopy(path)),
            Request::Switcher { mode: crate::switcher::Mode::Apps } => Action::AppSwitcher,
            Request::Switcher { mode: crate::switcher::Mode::Windows } => Action::WindowSwitcher,
            Request::FocusWindow { id } => Action::FocusWindow(id),
            Request::FocusApp { app_id } => Action::FocusApp(app_id),
            Request::DeleteScreenshot { path } => Action::DeleteScreenshot(path),
            Request::NightLight { on, temperature } => {
                Action::NightLight(crate::night_light::NightLight { on, temperature })
            }
            Request::Quit => Action::Quit,
            Request::NextWindowLayout { output } => Action::NextWindowLayout(output),
            Request::Restart => Action::Restart,
            Request::IdentifyOutputs => Action::IdentifyOutputs,
            Request::Capture { region, pointer, save } => {
                Action::Capture(crate::screenshot::Shot { region, pointer, save })
            }
            Request::WindowsList => Action::Shell(ShellRequest::WindowsList),
            Request::SnapPicker { under_pointer } => Action::SnapPicker(under_pointer),
            Request::PauseShortcuts { .. } => unreachable!("handled per client in ipc_request"),
        }
    }
}

/// The wire name of a shell action, e.g. `screenshot_region`.
pub fn action_name(action: &Action) -> Option<&'static str> {
    Some(match action {
        Action::AppLauncher => "app_launcher",
        Action::QuickSettings => "quick_settings",
        Action::Lock => "lock",
        Action::IdentifyOutputs => "identify_outputs",
        Action::Media(name) => name,
        Action::Notifications => "notifications",
        Action::Clipboard => "clipboard",
        Action::Note => "note",
        Action::ReportSnapshot => "report_snapshot",
        Action::Agenda => "agenda",
        Action::Music => "music",
        Action::ScreenshotRegion => "screenshot_region",
        _ => return None,
    })
}

/// Splits a byte stream into complete lines, keeping any partial tail.
#[derive(Debug, Default)]
pub struct LineBuffer {
    buf: Vec<u8>,
}

/// Lines longer than this are dropped: no legitimate request is anywhere near it.
pub const MAX_LINE: usize = 64 * 1024;

impl LineBuffer {
    /// Appends bytes and returns the complete lines (without '\n').
    /// Returns Err if a line grows past MAX_LINE; the buffer is then cleared.
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<String>, String> {
        self.buf.extend_from_slice(bytes);
        let mut lines = Vec::new();
        while let Some(i) = self.buf.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=i).collect();
            let line = String::from_utf8_lossy(&line[..line.len() - 1]).trim().to_string();
            if !line.is_empty() {
                lines.push(line);
            }
        }
        if self.buf.len() > MAX_LINE {
            self.buf.clear();
            return Err("line too long".into());
        }
        Ok(lines)
    }
}

struct Client {
    id: u64,
    stream: UnixStream,
}

/// The listening socket and connected shells.
pub struct Server {
    pub path: PathBuf,
    clients: Vec<Client>,
    next_id: u64,
    /// Last workspaces/focus events sent, so unchanged state isn't re-sent.
    last_workspaces: Option<Event>,
    last_focus: Option<Event>,
    last_windows: Option<Event>,
    last_switcher: Option<Event>,
    last_clipboard: Option<Event>,
    last_screenshots: Option<Event>,
    last_lock: Option<Event>,
    last_island: Option<Event>,
    last_outputs: Option<Event>,
    last_shortcuts: Option<Event>,
    last_keyboard: Option<Event>,
    last_snap_picker: Option<Event>,
}

impl Server {
    /// Binds `path` (replacing a stale socket) and registers it with the event loop.
    pub fn bind(path: &Path, handle: &LoopHandle<'static, WhiteRose>) -> std::io::Result<Server> {
        let _ = std::fs::remove_file(path);
        let listener = UnixListener::bind(path)?;
        listener.set_nonblocking(true)?;
        handle
            .insert_source(Generic::new(listener, Interest::READ, Mode::Level), |_, listener, state| {
                loop {
                    match listener.accept() {
                        Ok((stream, _)) => state.ipc_accept(stream),
                        Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                        Err(e) => {
                            tracing::warn!(%e, "ipc accept failed");
                            break;
                        }
                    }
                }
                Ok(PostAction::Continue)
            })
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        tracing::info!(path = %path.display(), "shell ipc listening");
        Ok(Server {
            path: path.to_path_buf(),
            clients: Vec::new(),
            next_id: 0,
            last_workspaces: None,
            last_focus: None,
            last_windows: None,
            last_switcher: None,
            last_clipboard: None,
            last_screenshots: None,
            last_lock: None,
            last_island: None,
            last_outputs: None,
            last_shortcuts: None,
            last_keyboard: None,
            last_snap_picker: None,
        })
    }

    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    fn send_to(client: &mut Client, line: &str) -> bool {
        // Messages are tiny; a client too slow to take one is dropped rather
        // than allowed to stall the compositor.
        match client.stream.write_all(line.as_bytes()) {
            Ok(()) => true,
            Err(e) => {
                tracing::info!(id = client.id, %e, "dropping ipc client");
                false
            }
        }
    }

    pub fn broadcast(&mut self, event: &Event) {
        let line = event.to_line();
        self.clients.retain_mut(|c| Self::send_to(c, &line));
    }

    /// Broadcasts state events only when they differ from the last one sent.
    pub fn update(&mut self, event: Event) {
        let slot = match event {
            Event::Workspaces { .. } => &mut self.last_workspaces,
            Event::Focus { .. } => &mut self.last_focus,
            Event::Windows { .. } => &mut self.last_windows,
            Event::Switcher { .. } => &mut self.last_switcher,
            Event::Clipboard { .. } => &mut self.last_clipboard,
            Event::Screenshots { .. } => &mut self.last_screenshots,
            Event::Lock { .. } => &mut self.last_lock,
            Event::Island { .. } => &mut self.last_island,
            Event::Outputs { .. } => &mut self.last_outputs,
            Event::Shortcuts { .. } => &mut self.last_shortcuts,
            Event::Keyboard { .. } => &mut self.last_keyboard,
            Event::SnapPicker { .. } => &mut self.last_snap_picker,
            Event::Action { .. } | Event::Screenshot { .. } | Event::ScreenshotFailed { .. } | Event::Layers { .. } => {
                self.broadcast(&event);
                return;
            }
        };
        if slot.as_ref() != Some(&event) {
            *slot = Some(event.clone());
            self.broadcast(&event);
        }
    }

    /// Sends `event` to one client.
    pub fn send(&mut self, id: u64, event: &Event) {
        let line = event.to_line();
        self.clients.retain_mut(|c| c.id != id || Self::send_to(c, &line));
    }

    fn remove(&mut self, id: u64) {
        self.clients.retain(|c| c.id != id);
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

impl WhiteRose {
    fn ipc_accept(&mut self, stream: UnixStream) {
        // Bring the new shell up to date immediately.
        let snapshot = [
            self.workspaces_event(),
            self.focus_event(),
            self.windows_event(),
            self.island_event(),
            self.outputs_event(),
            Event::Lock { locked: self.is_locked() },
            self.shortcuts_event(),
            self.keyboard_event(),
        ];
        let Some(server) = self.ipc.as_mut() else { return };
        let (Ok(()), Ok(reader)) = (stream.set_nonblocking(true), stream.try_clone()) else { return };
        let id = server.next_id;
        server.next_id += 1;
        let mut client = Client { id, stream };

        for e in &snapshot {
            if !Server::send_to(&mut client, &e.to_line()) {
                return;
            }
        }
        server.clients.push(client);
        // Everyone connected now has this state; don't send it again unless it changes.
        let [ws, focus, windows, island, outputs, lock, shortcuts, keyboard] = snapshot;
        server.last_keyboard = Some(keyboard);
        server.last_shortcuts = Some(shortcuts);
        server.last_outputs = Some(outputs);
        server.last_lock = Some(lock);
        server.last_island = Some(island);
        server.last_workspaces = Some(ws);
        server.last_focus = Some(focus);
        server.last_windows = Some(windows);
        tracing::info!(id, "shell connected");
        // Health check (whiterose-launch): the shell is up.
        crate::ready::mark("shell");

        let mut lines = LineBuffer::default();
        let inserted = self.loop_handle.insert_source(
            Generic::new(reader, Interest::READ, Mode::Level),
            move |_, reader, state| {
                let mut chunk = [0u8; 4096];
                loop {
                    // Safety: the stream is owned by this source and outlives the call.
                    match unsafe { reader.get_mut() }.read(&mut chunk) {
                        Ok(0) => {
                            state.ipc_disconnect(id);
                            return Ok(PostAction::Remove);
                        }
                        Ok(n) => match lines.push(&chunk[..n]) {
                            Ok(ls) => ls.iter().for_each(|l| state.ipc_request(id, l)),
                            Err(e) => {
                                tracing::warn!(id, e, "closing ipc client");
                                state.ipc_disconnect(id);
                                return Ok(PostAction::Remove);
                            }
                        },
                        Err(e) if e.kind() == ErrorKind::WouldBlock => return Ok(PostAction::Continue),
                        Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                        Err(_) => {
                            state.ipc_disconnect(id);
                            return Ok(PostAction::Remove);
                        }
                    }
                }
            },
        );
        if inserted.is_err() {
            self.ipc_disconnect(id);
        }
    }

    fn ipc_disconnect(&mut self, id: u64) {
        if let Some(server) = self.ipc.as_mut() {
            server.remove(id);
            tracing::info!(id, "shell disconnected");
        }
        // Settings closed (or crashed) while recording a shortcut.
        if self.shortcuts_paused == Some(id) {
            self.shortcuts_paused = None;
        }
    }

    fn ipc_request(&mut self, id: u64, line: &str) {
        if self.debug_pointer(line) || self.debug_key(line) || self.debug_layers(id, line) {
            return;
        }
        match Request::parse(line) {
            Ok(Request::PauseShortcuts { on }) => {
                self.shortcuts_paused = if on { Some(id) } else { self.shortcuts_paused.filter(|&p| p != id) };
                tracing::debug!(id, on, "shortcuts paused");
            }
            Ok(req) => {
                // Requests never come with modifiers held down.
                self.shortcut_mods = crate::keybind::Mods::NONE;
                self.run_action(req.into_action())
            }
            Err(e) => tracing::warn!(id, e, "ignoring ipc request"),
        }
    }

    /// Development only (`WHITEROSE_DEBUG_INPUT=1`): drives the pointer like
    /// a real mouse, for reproducing UI problems without a person clicking:
    /// `{"debug_pointer":{"x":720,"y":60,"button":"press"}}` (button optional:
    /// "press" or "release", or "right-press" / "right-release"); `"scroll":3`
    /// turns the wheel three clicks down (negative: up). Returns true if the
    /// line was such a request.
    fn debug_pointer(&mut self, line: &str) -> bool {
        #[derive(Deserialize)]
        struct P {
            x: f64,
            y: f64,
            button: Option<String>,
            scroll: Option<i32>,
        }
        #[derive(Deserialize)]
        struct Req {
            debug_pointer: P,
        }
        if !line.contains("\"debug_pointer\"") || std::env::var_os("WHITEROSE_DEBUG_INPUT").is_none() {
            return false;
        }
        let Ok(Req { debug_pointer: p }) = serde_json::from_str(line) else { return false };
        let time = (std::time::Instant::now() - self.start_time).as_millis() as u32;
        self.on_pointer_motion((p.x, p.y).into(), time);
        match p.button.as_deref() {
            Some("press") => self.on_pointer_button(0x110, smithay::backend::input::ButtonState::Pressed, time),
            Some("release") => self.on_pointer_button(0x110, smithay::backend::input::ButtonState::Released, time),
            Some("right-press") => self.on_pointer_button(0x111, smithay::backend::input::ButtonState::Pressed, time),
            Some("right-release") => {
                self.on_pointer_button(0x111, smithay::backend::input::ButtonState::Released, time)
            }
            _ => {}
        }
        if let Some(clicks) = p.scroll.filter(|&c| c != 0) {
            use smithay::{
                backend::input::{Axis, AxisSource},
                input::pointer::AxisFrame,
            };
            let frame = AxisFrame::new(time)
                .source(AxisSource::Wheel)
                .value(Axis::Vertical, 15.0 * clicks as f64)
                .v120(Axis::Vertical, 120 * clicks);
            let pointer = self.seat.get_pointer().expect("seat has a pointer");
            pointer.axis(self, frame);
            pointer.frame(self);
        }
        true
    }

    /// Development only, like debug_pointer: `{"debug_layers":true}` answers
    /// this client with every layer surface and where it is (UI tests click
    /// the bar's things at the bar's place plus their place in it).
    fn debug_layers(&mut self, id: u64, line: &str) -> bool {
        if !line.contains("\"debug_layers\"") || std::env::var_os("WHITEROSE_DEBUG_INPUT").is_none() {
            return false;
        }
        let event = self.layers_event();
        if let Some(server) = self.ipc.as_mut() {
            server.send(id, &event);
        }
        true
    }

    pub fn layers_event(&self) -> Event {
        use smithay::wayland::shell::wlr_layer::Layer;
        let mut layers = Vec::new();
        for output in self.space.outputs() {
            let at = self.space.output_geometry(output).map(|g| g.loc).unwrap_or_default();
            let map = smithay::desktop::layer_map_for_output(output);
            for l in map.layers() {
                let Some(g) = map.layer_geometry(l) else { continue };
                layers.push(LayerInfo {
                    namespace: l.namespace().to_string(),
                    output: output.name(),
                    layer: match l.layer() {
                        Layer::Background => "background",
                        Layer::Bottom => "bottom",
                        Layer::Top => "top",
                        Layer::Overlay => "overlay",
                    }
                    .into(),
                    geometry: [at.x + g.loc.x, at.y + g.loc.y, g.size.w, g.size.h],
                });
            }
        }
        Event::Layers { layers }
    }

    /// Development only, like debug_pointer: `{"debug_key":{"code":133,"down":true}}`
    /// presses or releases a key (xkb keycode, evdev + 8) as the keyboard would.
    fn debug_key(&mut self, line: &str) -> bool {
        #[derive(Deserialize)]
        struct K {
            code: u32,
            down: bool,
        }
        #[derive(Deserialize)]
        struct Req {
            debug_key: K,
        }
        if !line.contains("\"debug_key\"") || std::env::var_os("WHITEROSE_DEBUG_INPUT").is_none() {
            return false;
        }
        let Ok(Req { debug_key: k }) = serde_json::from_str(line) else { return false };
        let time = (std::time::Instant::now() - self.start_time).as_millis() as u32;
        let state = if k.down {
            smithay::backend::input::KeyState::Pressed
        } else {
            smithay::backend::input::KeyState::Released
        };
        self.on_key(smithay::input::keyboard::Keycode::new(k.code), state, time);
        true
    }

    pub fn workspaces_event(&self) -> Event {
        let count = self.workspaces.count();
        let active = self.workspaces.active();
        Event::Workspaces {
            active: active as u8 + 1,
            count: count as u8,
            occupied: (0..count).filter(|&i| !self.workspaces.windows(i).is_empty()).map(|i| i as u8 + 1).collect(),
            layouts: self.space.outputs().map(|o| (o.name(), self.layout_of(active, o).as_str().to_string())).collect(),
        }
    }

    pub fn windows_event(&self) -> Event {
        let focused = self.focused_window();
        let windows = self
            .workspaces
            .mru()
            .iter()
            .map(|w| {
                let (app_id, title) = toplevel_ids(w);
                WindowInfo {
                    id: self.window_id(w),
                    app_id,
                    title,
                    workspace: self.workspaces.workspace_of(w).map(|i| i as u8 + 1).unwrap_or(0),
                    focused: Some(w) == focused.as_ref(),
                    geometry: self.space.element_geometry(w).map(|g| [g.loc.x, g.loc.y, g.size.w, g.size.h]),
                }
            })
            .collect();
        Event::Windows { windows }
    }

    pub fn focus_event(&self) -> Event {
        let (app_id, title) = self.focused_window().map(|w| w.ids()).unwrap_or_default();
        Event::Focus { app_id, title }
    }

    /// Pushes workspace and focus state to connected shells, if it changed.
    pub fn ipc_sync(&mut self) {
        let (ws, focus, windows) = (self.workspaces_event(), self.focus_event(), self.windows_event());
        if let Some(server) = self.ipc.as_mut() {
            server.update(ws);
            server.update(focus);
            server.update(windows);
        }
        self.island_sync();
        self.foreign_toplevel_refresh();
    }

    pub fn island_event(&self) -> Event {
        let windows: Vec<_> = self.space.elements().filter_map(|w| self.space.element_geometry(w)).collect();
        let covered = self
            .space
            .outputs()
            .filter(|o| {
                self.space.output_geometry(o).is_some_and(|g| island_covered(&windows, g, self.usable_area_on(o).loc.y))
            })
            .map(|o| o.name())
            .collect();
        let fullscreen = self.space.outputs().filter(|o| self.fullscreen_on(o).is_some()).map(|o| o.name()).collect();
        Event::Island { covered, fullscreen }
    }

    /// Re-checks the area under the island (windows mapped, moved, resized).
    /// Only sends anything when the answer changes.
    pub fn island_sync(&mut self) {
        let e = self.island_event();
        if let Some(server) = self.ipc.as_mut() {
            server.update(e);
        }
    }
}

/// Half the width and the depth of the area the idle island hangs into,
/// below the bar at the top centre (the island is 170 wide plus its ears).
pub const ISLAND_HALF_WIDTH: i32 = 120;
pub const ISLAND_DEPTH: i32 = 24;

/// Whether any window overlaps the area under the bar where the idle island
/// hangs, on `output`.
pub fn island_covered(
    windows: &[smithay::utils::Rectangle<i32, smithay::utils::Logical>],
    output: smithay::utils::Rectangle<i32, smithay::utils::Logical>,
    bar_bottom: i32,
) -> bool {
    let centre = output.loc.x + output.size.w / 2;
    let zone = smithay::utils::Rectangle::new(
        (centre - ISLAND_HALF_WIDTH, bar_bottom).into(),
        (2 * ISLAND_HALF_WIDTH, ISLAND_DEPTH).into(),
    );
    windows.iter().any(|w| w.overlaps(zone))
}

/// A window's (app_id, title).
pub fn toplevel_ids(w: &smithay::desktop::Window) -> (Option<String>, Option<String>) {
    w.ids()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_serialize_to_one_json_line() {
        let layouts = [("DP-1".to_string(), "dwindle".to_string())].into();
        let e = Event::Workspaces { active: 2, count: 9, occupied: vec![1, 2], layouts };
        assert_eq!(
            e.to_line(),
            "{\"event\":\"workspaces\",\"active\":2,\"count\":9,\"occupied\":[1,2],\"layouts\":{\"DP-1\":\"dwindle\"}}\n"
        );
        let e = Event::Action { name: "screenshot_region".into() };
        assert_eq!(e.to_line(), "{\"event\":\"action\",\"name\":\"screenshot_region\"}\n");
        let e = Event::Focus { app_id: None, title: Some("a \"quoted\"\nline".into()) };
        let line = e.to_line();
        assert_eq!(line.matches('\n').count(), 1, "newlines inside strings must be escaped: {line}");
    }

    #[test]
    fn parses_requests() {
        assert_eq!(Request::parse(r#"{"cmd":"workspace","n":3}"#), Ok(Request::Workspace { n: 3 }));
        assert_eq!(Request::parse(r#"{"cmd":"move_to_workspace","n":9}"#), Ok(Request::MoveToWorkspace { n: 9 }));
        assert_eq!(
            Request::parse(r#"{"cmd":"spawn","command":"foot"}"#),
            Ok(Request::Spawn { command: "foot".into() })
        );
        assert_eq!(Request::parse(r#"{"cmd":"close_window"}"#), Ok(Request::CloseWindow));
        // The screenshot tool: a region, copied, with the pointer.
        let shot = Request::parse(r#"{"cmd":"capture","region":[10,20,300,200],"pointer":true,"save":false}"#).unwrap();
        assert_eq!(
            shot.into_action(),
            Action::Capture(crate::screenshot::Shot { region: Some([10, 20, 300, 200]), pointer: true, save: false })
        );
        // Defaults: the screen, saved, no pointer.
        assert_eq!(
            Request::parse(r#"{"cmd":"capture"}"#).unwrap().into_action(),
            Action::Capture(crate::screenshot::Shot::default())
        );
        assert_eq!(Request::parse(r#"{"cmd":"screenshot"}"#), Ok(Request::Screenshot));
        assert_eq!(Request::parse(r#"{"cmd":"clipboard_paste","id":7}"#), Ok(Request::ClipboardPaste { id: 7 }));
        assert_eq!(Request::parse(r#"{"cmd":"clipboard_clear"}"#), Ok(Request::ClipboardClear));
        assert_eq!(
            Request::parse(r#"{"cmd":"night_light","on":true,"temperature":4000}"#),
            Ok(Request::NightLight { on: true, temperature: 4000 })
        );
        assert_eq!(
            Request::parse(r#"{"cmd":"night_light","on":false}"#),
            Ok(Request::NightLight { on: false, temperature: 6500 })
        );
        assert_eq!(Request::parse(r#"{"cmd":"quit"}"#).map(Request::into_action), Ok(Action::Quit));
        assert_eq!(Request::parse(r#"{"cmd":"restart"}"#).map(Request::into_action), Ok(Action::Restart));
        assert_eq!(
            Request::parse(r#"{"cmd":"screenshot_copy","path":"/p.png"}"#),
            Ok(Request::ScreenshotCopy { path: "/p.png".into() })
        );
        assert_eq!(
            Request::parse(r#"{"cmd":"switcher","mode":"windows"}"#),
            Ok(Request::Switcher { mode: crate::switcher::Mode::Windows })
        );
        assert_eq!(
            Request::parse(r#"{"cmd":"delete_screenshot","path":"/p.png"}"#),
            Ok(Request::DeleteScreenshot { path: "/p.png".into() })
        );
    }

    #[test]
    fn rejects_bad_requests() {
        for bad in [
            "",
            "nope",
            "{}",
            r#"{"cmd":"format_disk"}"#,
            r#"{"cmd":"workspace"}"#,
            r#"{"cmd":"workspace","n":-1}"#,
            r#"{"cmd":"workspace","n":300}"#,
        ] {
            assert!(Request::parse(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn requests_map_to_actions() {
        assert_eq!(Request::Workspace { n: 4 }.into_action(), Action::Workspace(4));
        assert_eq!(
            Request::parse(r#"{"cmd":"next_window_layout","output":"DP-1"}"#).map(Request::into_action),
            Ok(Action::NextWindowLayout(Some("DP-1".into())))
        );
        assert_eq!(
            Request::parse(r#"{"cmd":"next_window_layout"}"#).map(Request::into_action),
            Ok(Action::NextWindowLayout(None))
        );
        assert_eq!(Request::CloseWindow.into_action(), Action::CloseWindow);
    }

    #[test]
    fn island_is_covered_only_by_windows_right_under_it() {
        use smithay::utils::Rectangle;
        let out = Rectangle::new((0, 0).into(), (1440, 808).into());
        let r = |x, y, w, h| Rectangle::new((x, y).into(), (w, h).into());
        let bar = 26;
        assert!(island_covered(&[r(0, 26, 1440, 782)], out, bar), "maximized window");
        assert!(island_covered(&[r(600, 30, 400, 300)], out, bar), "title bar just under the island");
        assert!(!island_covered(&[r(600, 200, 400, 300)], out, bar), "lower down: island stays out");
        assert!(!island_covered(&[r(0, 26, 500, 782)], out, bar), "left half only (tiled)");
        assert!(island_covered(&[r(0, 26, 601, 782)], out, bar), "reaches into the island's span");
        assert!(!island_covered(&[], out, bar), "empty desktop");
        // A second output to the right: its own centre.
        let out2 = Rectangle::new((1440, 0).into(), (1920, 1080).into());
        assert!(island_covered(&[r(1440 + 900, 26, 200, 100)], out2, bar));
        assert!(!island_covered(&[r(600, 26, 400, 100)], out2, bar), "a window on the other output");
    }

    #[test]
    fn every_shell_action_has_a_wire_name_and_others_do_not() {
        let shell = [
            Action::AppLauncher,
            Action::QuickSettings,
            Action::Lock,
            Action::IdentifyOutputs,
            Action::Notifications,
            Action::ScreenshotRegion,
        ];
        for a in &shell {
            assert!(a.is_shell());
            assert!(action_name(a).is_some(), "{a:?} has no wire name");
        }
        assert_eq!(action_name(&Action::CloseWindow), None);
        assert_eq!(action_name(&Action::Workspace(1)), None);
    }

    #[test]
    fn line_buffer_handles_partial_and_multiple_lines() {
        let mut b = LineBuffer::default();
        assert_eq!(b.push(b"{\"a\":").unwrap(), Vec::<String>::new());
        assert_eq!(b.push(b"1}\n{\"b\":2}\n{\"c\"").unwrap(), vec!["{\"a\":1}", "{\"b\":2}"]);
        assert_eq!(b.push(b":3}\n").unwrap(), vec!["{\"c\":3}"]);
        assert_eq!(b.push(b"\n\n  \n").unwrap(), Vec::<String>::new(), "blank lines are skipped");
    }

    #[test]
    fn line_buffer_rejects_runaway_lines() {
        let mut b = LineBuffer::default();
        assert!(b.push(&vec![b'x'; MAX_LINE + 1]).is_err());
        // Recovers afterwards.
        assert_eq!(b.push(b"ok\n").unwrap(), vec!["ok"]);
    }
}
