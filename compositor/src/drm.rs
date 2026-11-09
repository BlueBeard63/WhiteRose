//! Real-hardware backend: WhiteRose as the desktop itself, started from a
//! text console (or a display manager).
//!
//! - Session: libseat (logind) opens the GPU and input devices for us and
//!   tells us when we lose them (switching to another console) and get them
//!   back. Ctrl+Alt+F1-F12 switch consoles.
//! - Display: one GPU through DRM/KMS with GBM buffers. Each connected screen
//!   gets a CRTC; monitors come and go with hotplug. Each screen is
//!   identified by its EDID (see monitor_id) so its settings follow it.
//! - Drawing is on demand, as in the nested backend: a screen is redrawn only
//!   when something changed, at most once per refresh (the next frame waits
//!   for the vblank of the last). Smithay's DRM compositor puts the cursor and
//!   suitable windows straight on hardware planes.
//! - Night Light loads the colour into the screen's gamma table: no extra
//!   drawing at all.
//! - Input: libinput.

use std::{cell::RefCell, collections::HashMap, path::Path, rc::Rc, time::Duration};

use smithay::{
    backend::{
        allocator::{
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
            Fourcc,
        },
        drm::{
            compositor::FrameFlags,
            exporter::gbm::GbmFramebufferExporter,
            output::{DrmOutput, DrmOutputManager, DrmOutputRenderElements},
            DrmDevice, DrmDeviceFd, DrmEvent, DrmNode, NodeType, VrrSupport,
        },
        egl::{EGLContext, EGLDevice, EGLDisplay},
        input::InputEvent,
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{gles::GlesRenderer, ImportDma},
        session::{libseat::LibSeatSession, Event as SessionEvent, Session},
        udev::{all_gpus, primary_gpu, UdevBackend, UdevEvent},
    },
    desktop::layer_map_for_output,
    output::{Mode as WlMode, Output, PhysicalProperties, Subpixel},
    reexports::{
        calloop::{
            ping::make_ping,
            timer::{TimeoutAction, Timer},
            EventLoop,
        },
        drm::control::{connector, crtc, Device as _, ModeTypeFlags},
        input::{DeviceCapability, Libinput},
        rustix::fs::OFlags,
        wayland_server::backend::GlobalId,
    },
    utils::DeviceFd,
    wayland::dmabuf::DmabufFeedbackBuilder,
};
use smithay_drm_extras::drm_scanner::{DrmScanEvent, DrmScanner};

use crate::{
    cursor::Cursors,
    display_config::ModeInfo,
    monitor_id,
    night_light::NightLight,
    outputs::{self, OutputInfo},
    render::{self, OutputElement},
    state::WhiteRose,
};

type Allocator = GbmAllocator<DrmDeviceFd>;
type Exporter = GbmFramebufferExporter<DrmDeviceFd>;
type Manager = DrmOutputManager<Allocator, Exporter, (), DrmDeviceFd>;
type Surface = DrmOutput<Allocator, Exporter, (), DrmDeviceFd>;

/// 8-bit first: 10-bit buffers cost more bandwidth (and battery) and
/// nothing in the desktop needs them yet.
const COLOR_FORMATS: [Fourcc; 2] = [Fourcc::Argb8888, Fourcc::Abgr8888];

/// One connected screen.
struct Screen {
    output: Output,
    global: GlobalId,
    connector: connector::Handle,
    connector_name: String,
    edid: Option<monitor_id::Edid>,
    path: String,
    /// The connector's modes, as the hardware takes them.
    modes: Vec<smithay::reexports::drm::control::Mode>,
    surface: Option<Surface>,
    /// The mode the surface runs in.
    mode: smithay::reexports::drm::control::Mode,
    /// A frame is queued; the next one waits for its vblank.
    pending: bool,
    /// Something changed while a frame was pending.
    dirty: bool,
    /// Lit (false once turned off for being disabled in the settings).
    lit: bool,
    vrr_support: VrrSupport,
    /// The Night Light last loaded into the gamma table (None: unknown, as
    /// after starting or coming back from another console).
    gamma: Option<NightLight>,
    gamma_size: usize,
    /// Frame callbacks are due after a frame with nothing to show.
    callbacks_armed: bool,
}

pub struct Backend {
    session: LibSeatSession,
    libinput: Libinput,
    node: DrmNode,
    renderer: GlesRenderer,
    manager: Manager,
    scanner: DrmScanner,
    screens: HashMap<crtc::Handle, Screen>,
    cursors: Cursors,
    active: bool,
}

type Shared = Rc<RefCell<Backend>>;

pub fn init_drm(
    event_loop: &mut EventLoop<'static, WhiteRose>,
    state: &mut WhiteRose,
) -> Result<(), Box<dyn std::error::Error>> {
    let (session, notifier) = LibSeatSession::new().map_err(|e| format!("no session (libseat): {e:?}"))?;
    let seat = session.seat();

    // One GPU: the boot GPU, else the first one.
    let gpu_path = primary_gpu(&seat)?
        .or_else(|| all_gpus(&seat).ok().and_then(|g| g.into_iter().next()))
        .ok_or("no GPU found")?;
    let node = DrmNode::from_path(&gpu_path)?;
    tracing::info!(gpu = %gpu_path.display(), "using GPU");

    let libinput = libinput(&session, &seat)?;
    let cursor = crate::cursor::theme();
    tracing::info!(theme = cursor.0, size = cursor.1, "cursor");
    let (backend, drm_notifier) = open_gpu(session, libinput, node, &gpu_path, &cursor)?;
    state.cursor_theme = Some(cursor);
    let backend: Shared = Rc::new(RefCell::new(backend));

    init_dmabuf(&backend, state);
    state.draws_cursor = true;
    state.caps =
        outputs::Capabilities { modes: true, rotation: true, position: true, disable: true, vrr: false, pointer: true };

    // vblanks: the frame is on screen; draw the next if something changed.
    let b = backend.clone();
    event_loop.handle().insert_source(drm_notifier, move |event, _, state| match event {
        DrmEvent::VBlank(crtc) => vblank(&b, state, crtc),
        DrmEvent::Error(err) => tracing::warn!(?err, "DRM error"),
    })?;

    // Input.
    let input = LibinputInputBackend::new(backend.borrow().libinput.clone());
    event_loop.handle().insert_source(input, |mut event, _, state| {
        if let InputEvent::DeviceAdded { device } = &mut event {
            configure_device(device);
        }
        state.process_input_event(event);
    })?;

    // Losing the session (another console) and getting it back.
    let b = backend.clone();
    event_loop.handle().insert_source(notifier, move |event, _, state| match event {
        SessionEvent::PauseSession => {
            tracing::info!("session paused");
            let mut b = b.borrow_mut();
            b.active = false;
            b.libinput.suspend();
            b.manager.pause();
            for s in b.screens.values_mut() {
                s.pending = false;
            }
        }
        SessionEvent::ActivateSession => {
            tracing::info!("session resumed");
            {
                let mut b = b.borrow_mut();
                b.active = true;
                if let Err(err) = b.libinput.resume() {
                    tracing::warn!(?err, "libinput resume failed");
                }
                if let Err(err) = b.manager.activate(false) {
                    tracing::warn!(?err, "DRM resume failed");
                }
                for s in b.screens.values_mut() {
                    // Whoever had the screens may have changed their gamma.
                    s.gamma = None;
                    s.lit = true;
                    if let Some(surface) = &s.surface {
                        surface.reset_buffers();
                    }
                }
            }
            // Screens may have been plugged in or out meanwhile.
            scan(&b, state);
            redraw_all(&b, state);
        }
    })?;

    // Hotplug.
    let udev = UdevBackend::new(&seat)?;
    let b = backend.clone();
    event_loop.handle().insert_source(udev, move |event, _, state| match event {
        UdevEvent::Changed { device_id } if device_id == b.borrow().node.dev_id() => scan(&b, state),
        UdevEvent::Added { path, .. } => tracing::info!(path = %path.display(), "another GPU appeared (not used)"),
        _ => {}
    })?;

    // Redraw requests.
    let (ping, ping_source) = make_ping()?;
    let b = backend.clone();
    event_loop.handle().insert_source(ping_source, move |_, _, state| redraw_all(&b, state))?;
    state.redraw = Some(ping);

    // Ctrl+Alt+Fn.
    let b = backend.clone();
    state.vt_switch = Some(Box::new(move |vt| {
        if let Err(err) = b.borrow_mut().session.change_vt(vt) {
            tracing::warn!(vt, ?err, "console switch failed");
        }
    }));

    scan(&backend, state);
    redraw_all(&backend, state);
    Ok(())
}

fn libinput(session: &LibSeatSession, seat: &str) -> Result<Libinput, Box<dyn std::error::Error>> {
    let mut li = Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(session.clone().into());
    li.udev_assign_seat(seat).map_err(|()| "libinput couldn't take the seat")?;
    Ok(li)
}

/// Tap to click on touchpads, as every laptop user expects.
fn configure_device(device: &mut smithay::reexports::input::Device) {
    if device.has_capability(DeviceCapability::Pointer) && device.config_tap_finger_count() > 0 {
        let _ = device.config_tap_set_enabled(true);
    }
}

/// Opens the GPU: KMS device, GBM buffers, a GLES renderer.
fn open_gpu(
    mut session: LibSeatSession,
    libinput: Libinput,
    node: DrmNode,
    path: &Path,
    cursor: &(String, u32),
) -> Result<(Backend, smithay::backend::drm::DrmDeviceNotifier), Box<dyn std::error::Error>> {
    let fd = session.open(path, OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK).map_err(|e| {
        format!(
            "can't open {}: {e:?}. Another desktop is probably using the screens: \
                 start WhiteRose from a text console (Ctrl+Alt+F3 and log in), not from inside a desktop",
            path.display()
        )
    })?;
    let fd = DrmDeviceFd::new(DeviceFd::from(fd));
    let (drm, notifier) = DrmDevice::new(fd.clone(), true)?;
    let gbm = GbmDevice::new(fd)?;
    let egl = unsafe { EGLDisplay::new(gbm.clone())? };
    let render_node =
        EGLDevice::device_for_display(&egl).ok().and_then(|d| d.try_get_render_node().ok().flatten()).unwrap_or(node);
    let renderer = unsafe { GlesRenderer::new(EGLContext::new(&egl)?)? };
    let render_formats = renderer.egl_context().dmabuf_render_formats().clone();
    let manager = DrmOutputManager::new(
        drm,
        GbmAllocator::new(gbm.clone(), GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT),
        GbmFramebufferExporter::new(gbm.clone(), Some(render_node)),
        Some(gbm),
        COLOR_FORMATS,
        render_formats,
    );
    let backend = Backend {
        libinput,
        session,
        node: render_node.node_with_type(NodeType::Render).and_then(Result::ok).unwrap_or(node),
        renderer,
        manager,
        scanner: DrmScanner::new(),
        screens: HashMap::new(),
        cursors: Cursors::new(&cursor.0, cursor.1),
        active: true,
    };
    Ok((backend, notifier))
}

/// linux-dmabuf v4, so GTK and Qt render on the GPU and hand us their buffers.
fn init_dmabuf(backend: &Shared, state: &mut WhiteRose) {
    let (formats, node) = {
        let b = backend.borrow();
        (b.renderer.dmabuf_formats(), b.node)
    };
    let dh = state.display_handle.clone();
    let global = match DmabufFeedbackBuilder::new(node.dev_id(), formats.clone()).build() {
        Ok(feedback) => state.dmabuf_state.create_global_with_default_feedback::<WhiteRose>(&dh, &feedback),
        Err(err) => {
            tracing::warn!(?err, "dmabuf feedback failed, using v3");
            state.dmabuf_state.create_global::<WhiteRose>(&dh, formats)
        }
    };
    state.dmabuf_global = Some(global);
    let b = backend.clone();
    state.dmabuf_importer = Some(Box::new(move |dmabuf| b.borrow_mut().renderer.import_dmabuf(dmabuf, None).is_ok()));
}

/// Reads a connector property blob (EDID) or string-ish value (PATH).
fn connector_blob(dev: &DrmDevice, conn: connector::Handle, name: &str) -> Option<Vec<u8>> {
    let props = dev.get_properties(conn).ok()?;
    let (handles, values) = props.as_props_and_values();
    handles.iter().zip(values).find_map(|(h, v)| {
        let info = dev.get_property(*h).ok()?;
        (info.name().to_str() == Ok(name) && *v != 0).then(|| dev.get_property_blob(*v).ok()).flatten()
    })
}

fn mode_info(m: &smithay::reexports::drm::control::Mode) -> ModeInfo {
    let wl = WlMode::from(*m);
    ModeInfo {
        width: wl.size.w,
        height: wl.size.h,
        refresh: wl.refresh,
        preferred: m.mode_type().contains(ModeTypeFlags::PREFERRED),
    }
}

/// Looks for screens plugged in or out, and sets up the new ones.
fn scan(backend: &Shared, state: &mut WhiteRose) {
    let events = {
        let mut b = backend.borrow_mut();
        let b = &mut *b;
        match b.scanner.scan_connectors(b.manager.device()) {
            Ok(r) => r.into_iter().collect::<Vec<_>>(),
            Err(err) => {
                tracing::warn!(?err, "connector scan failed");
                return;
            }
        }
    };
    if events.is_empty() {
        return;
    }
    let mut added = Vec::new();
    for event in events {
        match event {
            DrmScanEvent::Connected { connector, crtc: Some(crtc) } => {
                if let Some(s) = connected(backend, state, &connector, crtc) {
                    added.push(crtc);
                    backend.borrow_mut().screens.insert(crtc, s);
                }
            }
            DrmScanEvent::Disconnected { crtc: Some(crtc), connector } => {
                let removed = backend.borrow_mut().screens.remove(&crtc);
                if let Some(s) = removed {
                    tracing::info!(connector = s.connector_name, "screen unplugged");
                    state.outputs.retain(|o| o != &s.output);
                    state.space.unmap_output(&s.output);
                    state.display_handle.remove_global::<WhiteRose>(s.global);
                    drop(s.surface);
                }
                let _ = connector;
            }
            _ => {}
        }
    }

    identify(backend);
    // Lay everything out by the saved settings (which picks the modes), then
    // light the new screens in the mode chosen for them.
    state.apply_displays();
    let mut guard = backend.borrow_mut();
    let b = &mut *guard;
    for crtc in added {
        let Some(s) = b.screens.get_mut(&crtc) else { continue };
        let want = outputs::info(&s.output).wanted_mode;
        let mode = want.and_then(|w| find_mode(&s.modes, &w)).unwrap_or(s.mode);
        s.output.change_current_state(Some(WlMode::from(mode)), None, None, None);
        match b.manager.initialize_output::<_, OutputElement>(
            crtc,
            mode,
            &[s.connector],
            &s.output,
            None,
            &mut b.renderer,
            &DrmOutputRenderElements::default(),
        ) {
            Ok(surface) => {
                s.mode = mode;
                s.vrr_support =
                    surface.with_compositor(|c| c.vrr_supported(s.connector)).unwrap_or(VrrSupport::NotSupported);
                s.surface = Some(surface);
            }
            Err(err) => tracing::warn!(connector = s.connector_name, ?err, "couldn't light the screen"),
        }
        layer_map_for_output(&s.output).arrange();
    }
    state.caps.vrr = b.screens.values().any(|s| s.vrr_support != VrrSupport::NotSupported);
    drop(guard);
    state.refit_windows();
    state.outputs_sync();
    state.schedule_redraw();
}

/// A screen was plugged in: its output (not yet lit).
fn connected(backend: &Shared, state: &mut WhiteRose, conn: &connector::Info, crtc: crtc::Handle) -> Option<Screen> {
    let b = backend.borrow();
    let dev = b.manager.device();
    let name = format!("{}-{}", conn.interface().as_str(), conn.interface_id());
    // Headsets (non-desktop) are for VR runtimes, not the desktop.
    if conn.modes().is_empty() {
        return None;
    }
    let edid = connector_blob(dev, conn.handle(), "EDID").and_then(|d| monitor_id::parse(&d));
    let path = connector_blob(dev, conn.handle(), "PATH")
        .map(|p| String::from_utf8_lossy(&p).trim_end_matches('\0').to_string())
        .unwrap_or_default();
    let modes = conn.modes().to_vec();
    let preferred =
        modes.iter().find(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED)).copied().unwrap_or(modes[0]);
    let (mm_w, mm_h) = conn.size().unwrap_or((0, 0));
    let (make, model) = match &edid {
        Some(e) => {
            (e.manufacturer.clone(), if e.name.is_empty() { format!("{:04x}", e.product) } else { e.name.clone() })
        }
        None => (String::new(), name.clone()),
    };
    let gamma_size = dev.get_crtc(crtc).map(|c| c.gamma_length() as usize).unwrap_or(0);
    drop(b);

    tracing::info!(connector = name, make, model, "screen connected");
    let output = Output::new(
        name.clone(),
        PhysicalProperties {
            size: (mm_w as i32, mm_h as i32).into(),
            subpixel: Subpixel::Unknown,
            make: make.clone(),
            model: model.clone(),
        },
    );
    let global = output.create_global::<WhiteRose>(&state.display_handle);
    output.set_preferred(WlMode::from(preferred));
    output.change_current_state(Some(WlMode::from(preferred)), None, None, None);
    let builtin = ["eDP", "LVDS", "DSI"].iter().any(|p| name.starts_with(p));
    outputs::set_info(
        &output,
        OutputInfo {
            id: format!("connector:{name}"),
            connector: name.clone(),
            make,
            model,
            builtin,
            physical_mm: (mm_w as i32, mm_h as i32),
            modes: modes.iter().map(mode_info).collect(),
            enabled: true,
            ..Default::default()
        },
    );
    state.outputs.push(output.clone());
    Some(Screen {
        output,
        global,
        connector: conn.handle(),
        connector_name: name,
        edid,
        path,
        modes,
        surface: None,
        mode: preferred,
        pending: false,
        dirty: true,
        lit: true,
        vrr_support: VrrSupport::NotSupported,
        gamma: None,
        gamma_size,
        callbacks_armed: false,
    })
}

/// Gives every screen its stable ID. Needs all of them at once: two
/// identical monitors are told apart by where they're plugged in.
fn identify(backend: &Shared) {
    let b = backend.borrow();
    let found: Vec<monitor_id::Found> = b
        .screens
        .values()
        .map(|s| monitor_id::Found { connector: s.connector_name.clone(), path: s.path.clone(), edid: s.edid.clone() })
        .collect();
    for (connector, id, strong) in monitor_id::identify(&found) {
        if let Some(s) = b.screens.values().find(|s| s.connector_name == connector) {
            let mut i = outputs::info(&s.output);
            i.id = id;
            i.strong = strong;
            outputs::set_info(&s.output, i);
        }
    }
}

fn find_mode(
    modes: &[smithay::reexports::drm::control::Mode],
    want: &ModeInfo,
) -> Option<smithay::reexports::drm::control::Mode> {
    modes.iter().copied().find(|m| {
        let i = mode_info(m);
        i.width == want.width && i.height == want.height && i.refresh == want.refresh
    })
}

fn redraw_all(backend: &Shared, state: &mut WhiteRose) {
    let crtcs: Vec<crtc::Handle> = backend.borrow().screens.keys().copied().collect();
    for crtc in crtcs {
        draw(backend, state, crtc);
    }
    // No screen is lit (all disabled, or none connected): nothing will ever
    // be presented, so a lock request mustn't wait for a frame.
    if !backend.borrow().screens.values().any(|s| s.lit && s.surface.is_some()) {
        state.frame_presented();
    }
}

fn vblank(backend: &Shared, state: &mut WhiteRose, crtc: crtc::Handle) {
    let again = {
        let mut b = backend.borrow_mut();
        let Some(s) = b.screens.get_mut(&crtc) else { return };
        if let Some(surface) = s.surface.as_mut() {
            if let Err(err) = surface.frame_submitted() {
                tracing::warn!(?err, "frame_submitted");
            }
        }
        s.pending = false;
        std::mem::take(&mut s.dirty)
    };
    // What the lock screen was waiting for: a locked frame on screen.
    state.frame_presented();
    if again {
        draw(backend, state, crtc);
    }
}

/// Brings one screen up to date with the settings (mode, on/off, VRR, Night
/// Light) and draws it if anything changed.
fn draw(backend: &Shared, state: &mut WhiteRose, crtc: crtc::Handle) {
    let mut guard = backend.borrow_mut();
    let b = &mut *guard;
    if !b.active {
        return;
    }
    let Some(s) = b.screens.get_mut(&crtc) else { return };
    if s.pending {
        s.dirty = true;
        return;
    }
    let Some(surface) = s.surface.as_mut() else { return };
    let info = outputs::info(&s.output);

    // Disabled in the settings, or everything idle: dark (DPMS off) until
    // enabled again or the next input.
    if !info.enabled || state.idle.screens_off {
        if s.lit {
            if let Err(err) = surface.with_compositor(|c| c.clear()) {
                tracing::warn!(?err, "couldn't turn the screen off");
            }
            s.lit = false;
        }
        return;
    }
    if !s.lit {
        // Coming back on: draw everything, not just what changed while dark.
        surface.reset_buffers();
    }
    s.lit = true;

    // A different mode asked for.
    if let Some(m) = info.wanted_mode.and_then(|w| find_mode(&s.modes, &w)).filter(|m| *m != s.mode) {
        match surface.use_mode(m, &mut b.renderer, &DrmOutputRenderElements::<_, OutputElement>::default()) {
            Ok(()) => {
                tracing::info!(connector = s.connector_name, mode = ?WlMode::from(m), "mode changed");
                s.mode = m;
                s.output.change_current_state(Some(WlMode::from(m)), None, None, None);
                layer_map_for_output(&s.output).arrange();
                drop(guard);
                state.refit_windows();
                state.outputs_sync();
                // Draw it with the new size.
                return draw(backend, state, crtc);
            }
            Err(err) => {
                tracing::warn!(connector = s.connector_name, ?err, "mode change failed");
                // Don't try again every frame.
                let mut i = info.clone();
                i.wanted_mode = None;
                outputs::set_info(&s.output, i);
            }
        }
    }

    if s.vrr_support != VrrSupport::NotSupported && surface.with_compositor(|c| c.vrr_enabled()) != info.vrr {
        if let Err(err) = surface.with_compositor(|c| c.use_vrr(info.vrr)) {
            tracing::warn!(?err, "VRR change failed");
        }
    }

    // Night Light, through the gamma table.
    if s.gamma != Some(state.night_light) && s.gamma_size > 1 {
        let lut = gamma_ramp(s.gamma_size, state.night_light.multiplier().unwrap_or([1.0; 3]));
        match b.manager.device().set_gamma(crtc, &lut[0], &lut[1], &lut[2]) {
            Ok(()) => s.gamma = Some(state.night_light),
            Err(err) => {
                tracing::warn!(?err, "gamma table refused");
                s.gamma = Some(state.night_light); // don't retry every frame
            }
        }
    }

    // The frame.
    let output = s.output.clone();
    let scale = output.current_scale().fractional_scale();
    let geo = state.space.output_geometry(&output);
    let pointer = state.seat.get_pointer().map(|p| p.current_location()).unwrap_or_default();
    let front = match geo {
        Some(g) if g.to_f64().contains(pointer) => {
            b.cursors.elements(&mut b.renderer, &mut state.cursor_status, pointer - g.loc.to_f64(), scale)
        }
        _ => Vec::new(),
    };
    let (elements, clear) = render::output_elements(&mut b.renderer, &output, state, front);
    let queued = match surface.render_frame(&mut b.renderer, &elements, clear, FrameFlags::DEFAULT) {
        Ok(r) if !r.is_empty => match surface.queue_frame(()) {
            Ok(()) => true,
            Err(err) => {
                tracing::warn!(?err, "queue_frame failed");
                false
            }
        },
        Ok(_) => false,
        Err(err) => {
            tracing::warn!(?err, "render failed");
            false
        }
    };
    s.pending = queued;
    let refresh = s.mode.vrefresh().max(1);
    let arm_callbacks = !queued && !std::mem::replace(&mut s.callbacks_armed, true);
    if queued {
        s.callbacks_armed = false;
    }

    // A screenshot comes from the output it's of (the region's, or the one
    // under the pointer), with the pointer drawn in if asked.
    if state.screenshot_pending && state.screenshot_output().as_ref() == Some(&output) {
        state.screenshot_pending = false;
        let pointer = match geo {
            Some(g) if g.to_f64().contains(pointer) => {
                b.cursors.elements(&mut b.renderer, &mut state.cursor_status, pointer - g.loc.to_f64(), scale)
            }
            _ => Vec::new(),
        };
        crate::screenshot::capture_and_save(&mut b.renderer, &output, state, pointer);
    }
    crate::toplevel_export::process(&mut b.renderer, &output, state);
    drop(guard);

    if queued {
        render::send_frame_callbacks(&output, state);
    } else if arm_callbacks {
        // Nothing new to show: clients waiting to draw get their callback a
        // frame later instead of at once, which would spin.
        let b = backend.clone();
        let _ = state.loop_handle.insert_source(
            Timer::from_duration(Duration::from_micros(1_000_000 / refresh as u64)),
            move |_, _, state| {
                if let Some(s) = b.borrow_mut().screens.get_mut(&crtc) {
                    s.callbacks_armed = false;
                }
                render::send_frame_callbacks(&output, state);
                TimeoutAction::Drop
            },
        );
    }
}

/// A gamma table that multiplies each channel by `mult`.
pub fn gamma_ramp(size: usize, mult: [f32; 3]) -> [Vec<u16>; 3] {
    let ramp = |m: f32| -> Vec<u16> {
        (0..size).map(|i| ((i as f64 / (size - 1) as f64) * 65535.0 * m as f64).round() as u16).collect()
    };
    [ramp(mult[0]), ramp(mult[1]), ramp(mult[2])]
}

#[cfg(test)]
mod tests {
    use super::gamma_ramp;

    #[test]
    fn gamma_ramp_is_linear_and_scaled() {
        let [r, g, b] = gamma_ramp(256, [1.0, 0.5, 0.0]);
        assert_eq!((r[0], r[255]), (0, 65535));
        assert_eq!(g[255], 32768);
        assert!(b.iter().all(|v| *v == 0));
        assert!(r.windows(2).all(|w| w[0] <= w[1]), "monotonic");
    }
}
