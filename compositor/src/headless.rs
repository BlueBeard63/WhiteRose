//! Headless backend (`--backend headless`): no screen, no GPU, no input
//! devices. One virtual 1280x800 output that nothing is drawn to; clients
//! are told their frames were shown at most 60 times a second, so they keep
//! drawing (in software, into shared memory) and stay responsive. Input
//! comes only through the debug pointer and keys (WHITEROSE_DEBUG_INPUT).
//! For the click-and-check UI tests in CI, where there is no desktop to
//! nest in.

use std::{cell::Cell, rc::Rc, time::Duration};

use smithay::{
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::calloop::{
        ping::make_ping,
        timer::{TimeoutAction, Timer},
        EventLoop,
    },
    utils::Transform,
};

use crate::{render, state::WhiteRose};

pub const WIDTH: i32 = 1280;
pub const HEIGHT: i32 = 800;

pub fn init_headless(
    event_loop: &mut EventLoop<'static, WhiteRose>,
    state: &mut WhiteRose,
) -> Result<(), Box<dyn std::error::Error>> {
    let mode = Mode { size: (WIDTH, HEIGHT).into(), refresh: 60_000 };
    let output = Output::new(
        "HEADLESS-1".to_string(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "WhiteRose".into(),
            model: "Headless".into(),
        },
    );
    let _global = output.create_global::<WhiteRose>(&state.display_handle);
    output.change_current_state(Some(mode), Some(Transform::Normal), None, Some((0, 0).into()));
    output.set_preferred(mode);
    state.space.map_output(&output, (0, 0));
    crate::outputs::set_info(
        &output,
        crate::outputs::OutputInfo {
            id: "connector:HEADLESS-1".into(),
            connector: "HEADLESS-1".into(),
            make: "WhiteRose".into(),
            model: "Headless".into(),
            modes: vec![crate::display_config::ModeInfo {
                width: WIDTH,
                height: HEIGHT,
                refresh: 60_000,
                preferred: true,
            }],
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
    // Nobody to be idle.
    state.idle.enabled = false;

    // "Something changed": the next tick tells clients their frame was shown.
    let wanted = Rc::new(Cell::new(true));
    let (ping, ping_source) = make_ping()?;
    let w = wanted.clone();
    event_loop.handle().insert_source(ping_source, move |_, _, _| w.set(true))?;
    state.redraw = Some(ping);
    event_loop.handle().insert_source(Timer::from_duration(Duration::from_millis(16)), move |_, _, state| {
        if wanted.replace(false) {
            render::send_frame_callbacks(&output, state);
        }
        // Nothing is drawn, so nothing of the desktop is on screen: a lock
        // is confirmed at the next tick (SUPER+L in the UI tests).
        state.frame_presented();
        TimeoutAction::ToDuration(Duration::from_millis(16))
    })?;
    Ok(())
}
