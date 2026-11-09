//! Display settings applied to outputs: preferences by monitor ID, profiles,
//! and a trial layout that reverts on its own when its time is up.

mod common;

use std::time::{Duration, Instant};

use common::*;
use whiterose_wm::{
    daemon_client::{now_ms, Trial},
    display_config::{Displays, ModeInfo},
    outputs::{self, Capabilities, OutputInfo},
};

fn setup() -> Headless {
    let mut h = Headless::new(true);
    let out = h.output.clone().unwrap();
    outputs::set_info(
        &out,
        OutputInfo {
            id: "builtin".into(),
            connector: "eDP-1".into(),
            builtin: true,
            modes: vec![ModeInfo { width: W, height: H, refresh: 60_000, preferred: true }],
            enabled: true,
            ..Default::default()
        },
    );
    h.state.outputs.push(out);
    h.state.caps =
        Capabilities { modes: true, rotation: true, position: true, disable: true, vrr: true, pointer: false };
    h
}

fn cfg(json: &str) -> Displays {
    serde_json::from_str(json).unwrap()
}

#[test]
fn settings_apply_to_the_output_with_that_id() {
    let mut h = setup();
    h.state.displays = cfg(r#"{"monitor":[{"id":"builtin","scale":1.5,"transform":"90"}]}"#);
    h.state.apply_displays();
    let out = h.output.clone().unwrap();
    assert_eq!(out.current_scale().fractional_scale(), 1.5);
    assert_eq!(outputs::transform_name(out.current_transform()), "90");
    let view = h.state.outputs_view();
    assert_eq!((view[0].id.as_str(), view[0].scale, view[0].transform.as_str()), ("builtin", 1.5, "90"));
    // Settings for another monitor change nothing here.
    h.state.displays = cfg(r#"{"monitor":[{"id":"edid:0d4384bf042b","scale":2}]}"#);
    h.state.apply_displays();
    assert_eq!(out.current_scale().fractional_scale(), 1.0);
}

#[test]
fn a_trial_layout_reverts_by_itself() {
    let mut h = setup();
    h.state.displays = cfg(r#"{"monitor":[{"id":"builtin","scale":1.25}]}"#);
    h.state.display_trial =
        Some(Trial { config: cfg(r#"{"monitor":[{"id":"builtin","scale":2}]}"#), until: now_ms() + 300 });
    h.state.apply_displays();
    let out = h.output.clone().unwrap();
    assert_eq!(out.current_scale().fractional_scale(), 2.0, "the trial shows first");
    assert!(matches!(h.state.outputs_event(), whiterose_wm::ipc::Event::Outputs { trial_until: Some(_), .. }));
    let deadline = Instant::now() + Duration::from_secs(3);
    while h.state.display_trial.is_some() {
        assert!(Instant::now() < deadline, "the trial never ended");
        h.dispatch();
    }
    assert_eq!(out.current_scale().fractional_scale(), 1.25, "back to the saved layout without anyone confirming");
}

#[test]
fn a_backend_that_cant_rotate_ignores_rotation() {
    let mut h = setup();
    h.state.caps =
        Capabilities { modes: false, rotation: false, position: false, disable: false, vrr: false, pointer: false };
    h.state.displays = cfg(
        r#"{"monitor":[{"id":"builtin","scale":1.5,"transform":"90"}],"profile":[{"output":[{"id":"builtin","position":[500,0]}]}]}"#,
    );
    h.state.apply_displays();
    let out = h.output.clone().unwrap();
    assert_eq!(out.current_scale().fractional_scale(), 1.5, "scale still applies (nested window)");
    assert_eq!(outputs::transform_name(out.current_transform()), "normal");
    assert_eq!(out.current_location(), (0, 0).into());
}
