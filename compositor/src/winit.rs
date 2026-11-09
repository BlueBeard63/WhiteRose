//! Nested backend: runs WhiteRose as a window inside another desktop, for
//! development. The DRM/libinput backend for real hardware comes later.
//!
//! Unlike Smithay's smallvil example, this only draws when something changed
//! (a client commit, input, resize) and skips the swap when nothing did.

use std::{cell::RefCell, rc::Rc};

use smithay::{
    backend::{
        egl::EGLDevice,
        renderer::{damage::OutputDamageTracker, gles::GlesRenderer, Frame, ImportDma, Renderer},
        winit::{self, WinitEvent, WinitGraphicsBackend},
    },
    desktop::layer_map_for_output,
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::calloop::{ping::make_ping, EventLoop},
    utils::Transform,
    wayland::dmabuf::DmabufFeedbackBuilder,
};

use crate::{render, screenshot, state::WhiteRose};

pub fn init_winit(
    event_loop: &mut EventLoop<'static, WhiteRose>,
    state: &mut WhiteRose,
) -> Result<(), Box<dyn std::error::Error>> {
    let (backend, winit) = winit::init::<GlesRenderer>()?;
    let backend = Rc::new(RefCell::new(backend));

    let mode = Mode { size: backend.borrow().window_size(), refresh: 60_000 };
    let output = Output::new(
        "winit".to_string(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "WhiteRose".into(),
            model: "Nested".into(),
        },
    );
    let _global = output.create_global::<WhiteRose>(&state.display_handle);
    output.change_current_state(Some(mode), Some(Transform::Flipped180), None, Some((0, 0).into()));
    output.set_preferred(mode);
    state.space.map_output(&output, (0, 0));
    // Nested, the one output is our window: only its scale is ours to change.
    let window_mode = |size: smithay::utils::Size<i32, smithay::utils::Physical>| crate::display_config::ModeInfo {
        width: size.w,
        height: size.h,
        refresh: 60_000,
        preferred: true,
    };
    crate::outputs::set_info(
        &output,
        crate::outputs::OutputInfo {
            id: "connector:winit".into(),
            connector: "winit".into(),
            make: "WhiteRose".into(),
            model: "Nested window".into(),
            modes: vec![window_mode(mode.size)],
            enabled: true,
            ..Default::default()
        },
    );
    state.outputs.push(output.clone());
    state.caps = crate::outputs::Capabilities {
        modes: false,
        rotation: false,
        position: false,
        disable: false,
        vrr: false,
        pointer: false,
    };

    init_dmabuf(&backend, state);
    // The host desktop handles idle (and sees input we never get).
    state.idle.enabled = false;

    // Let SUPER, ALT+TAB and PrtSc reach us instead of the host desktop.
    let mut host_inhibit = crate::host_inhibit::HostInhibit::request(backend.borrow().window());

    // Redraw requests from anywhere in the compositor arrive through this ping.
    let (ping, ping_source) = make_ping()?;
    let ping_backend = backend.clone();
    event_loop.handle().insert_source(ping_source, move |_, _, _| {
        ping_backend.borrow().window().request_redraw();
    })?;
    state.redraw = Some(ping);

    let mut damage_tracker = OutputDamageTracker::from_output(&output);
    let mut night = crate::night_light::Pass::default();
    let mut night_applied = state.night_light;
    let mut shown_cursor: Option<smithay::input::pointer::CursorIcon> = None;

    event_loop.handle().insert_source(winit, move |event, _, state| match event {
        WinitEvent::Resized { size, .. } => {
            output.change_current_state(Some(Mode { size, refresh: 60_000 }), None, None, None);
            let mut info = crate::outputs::info(&output);
            info.modes = vec![window_mode(size)];
            crate::outputs::set_info(&output, info);
            state.outputs_sync();
            // Panels anchored to edges (the bar) must follow the new size.
            layer_map_for_output(&output).arrange();
            state.refit_windows();
            state.schedule_redraw();
        }
        WinitEvent::Input(event) => state.process_input_event(event),
        WinitEvent::Redraw => {
            if let Some(h) = host_inhibit.as_mut() {
                h.dispatch();
            }
            // The host draws the pointer: ask it for the shape apps named.
            if let smithay::input::pointer::CursorImageStatus::Named(icon) = &state.cursor_status {
                if shown_cursor != Some(*icon) {
                    shown_cursor = Some(*icon);
                    backend.borrow().window().set_cursor(*icon);
                }
            }
            // Night Light toggled: the whole frame must be redrawn and re-tinted.
            if state.night_light != night_applied {
                night_applied = state.night_light;
                damage_tracker = OutputDamageTracker::from_output(&output);
            }
            render(&mut backend.borrow_mut(), &output, &mut damage_tracker, &mut night, state);
        }
        WinitEvent::CloseRequested => state.loop_signal.stop(),
        _ => (),
    })?;

    state.schedule_redraw();
    Ok(())
}

/// Advertises linux-dmabuf with the formats this GPU can import. Version 4
/// (with feedback naming the render node) is what Mesa's EGL needs for
/// GTK4/Qt to render on the GPU; fall back to v3 if the node can't be found.
fn init_dmabuf(backend: &Rc<RefCell<WinitGraphicsBackend<GlesRenderer>>>, state: &mut WhiteRose) {
    let (formats, render_node) = {
        let mut b = backend.borrow_mut();
        let formats = b.renderer().dmabuf_formats();
        let node = EGLDevice::device_for_display(b.renderer().egl_context().display())
            .and_then(|device| device.try_get_render_node());
        (formats, node)
    };
    let dh = state.display_handle.clone();
    let global = match render_node {
        Ok(Some(node)) => match DmabufFeedbackBuilder::new(node.dev_id(), formats.clone()).build() {
            Ok(feedback) => {
                tracing::info!(node = ?node, "linux-dmabuf v4 with feedback");
                state.dmabuf_state.create_global_with_default_feedback::<WhiteRose>(&dh, &feedback)
            }
            Err(err) => {
                tracing::warn!(?err, "dmabuf feedback failed, using v3");
                state.dmabuf_state.create_global::<WhiteRose>(&dh, formats)
            }
        },
        other => {
            tracing::warn!(?other, "no render node, using linux-dmabuf v3");
            state.dmabuf_state.create_global::<WhiteRose>(&dh, formats)
        }
    };
    state.dmabuf_global = Some(global);

    let importer = backend.clone();
    state.dmabuf_importer =
        Some(Box::new(move |dmabuf| importer.borrow_mut().renderer().import_dmabuf(dmabuf, None).is_ok()));
}

fn render(
    backend: &mut WinitGraphicsBackend<GlesRenderer>,
    output: &Output,
    damage_tracker: &mut OutputDamageTracker,
    night: &mut crate::night_light::Pass,
    state: &mut WhiteRose,
) {
    // The host compositor's EGL surface rejects buffer-age queries (BAD_SURFACE
    // on every frame, verified on Mesa/Intel), so nested mode always repaints
    // the whole frame, but only when something changed. The DRM backend
    // tracks buffer ages itself and gets true partial redraws.
    let age = 0;
    let damage = {
        let (renderer, mut framebuffer) = match backend.bind() {
            Ok(b) => b,
            Err(err) => {
                tracing::warn!(?err, "failed to bind backend");
                return;
            }
        };
        let damage = match render::compose(renderer, &mut framebuffer, output, state, damage_tracker, age) {
            Ok(damage) => damage,
            Err(err) => {
                tracing::warn!(err, "render failed");
                return;
            }
        };
        // Nested mode repaints the whole frame (age 0), so tinting all of it
        // after every drawn frame never tints anything twice.
        if let (Some(_), Some(tint)) = (&damage, state.night_light.multiplier()) {
            let size = output.current_mode().map(|m| m.size).unwrap_or_default();
            match renderer.render(&mut framebuffer, size, Transform::Normal) {
                Ok(mut frame) => {
                    // Safety: runs inside the frame, with its framebuffer bound.
                    let _ = frame.with_context(|gl| unsafe { night.apply(gl, tint) });
                    let _ = frame.finish();
                }
                Err(err) => tracing::warn!(?err, "night light pass failed"),
            }
        }
        damage
    };

    // Nothing changed on screen: skip the swap entirely.
    if let Some(damage) = damage {
        if let Err(err) = backend.submit(Some(&damage)) {
            tracing::warn!(?err, "submit failed");
        }
    }

    // Whatever is on screen now was drawn with the current lock state.
    state.frame_presented();

    // A screenshot was requested (PrtSc): capture what is now on screen.
    // (The host draws the pointer here, so a capture can't include it.)
    if state.screenshot_pending {
        state.screenshot_pending = false;
        screenshot::capture_and_save(backend.renderer(), output, state, Vec::new());
    }

    // Live window previews (dock hover, switcher).
    crate::toplevel_export::process(backend.renderer(), output, state);

    render::send_frame_callbacks(output, state);
}
