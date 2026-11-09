//! Idle: lock, then screens off, after no input; any input wakes them.

mod common;

use std::time::{Duration, Instant};

use common::*;

fn run_for(h: &mut Headless, d: Duration) {
    let end = Instant::now() + d;
    while Instant::now() < end {
        h.dispatch();
    }
}

#[test]
fn locks_then_turns_screens_off_and_input_wakes_them() {
    let mut h = Headless::new(true);
    h.state.idle.lock_after = Some(Duration::from_millis(40));
    h.state.idle.off_after = Some(Duration::from_millis(90));
    h.state.activity();

    run_for(&mut h, Duration::from_millis(60));
    assert!(h.state.idle.locked, "asked the shell to lock after 40 ms idle");
    assert!(!h.state.idle.screens_off, "screens still on at 60 ms");

    run_for(&mut h, Duration::from_millis(60));
    assert!(h.state.idle.screens_off, "screens off after 90 ms idle");

    h.state.on_pointer_motion((10.0, 10.0).into(), 0);
    assert!(!h.state.idle.screens_off && !h.state.idle.locked, "input wakes the screens and resets the idle stretch");

    // And it all happens again after the next idle stretch.
    run_for(&mut h, Duration::from_millis(120));
    assert!(h.state.idle.screens_off, "the timer re-armed after waking");
}

#[test]
fn never_does_nothing() {
    let mut h = Headless::new(true);
    h.state.idle.lock_after = None;
    h.state.idle.off_after = None;
    h.state.activity();
    run_for(&mut h, Duration::from_millis(50));
    assert!(!h.state.idle.locked && !h.state.idle.screens_off);
}

#[test]
fn a_window_keeping_the_screen_on_holds_idle_off() {
    use wayland_protocols::wp::idle_inhibit::zv1::client::zwp_idle_inhibit_manager_v1::ZwpIdleInhibitManagerV1;
    let mut h = Headless::new(true);
    h.state.idle.lock_after = Some(Duration::from_millis(40));
    h.state.idle.off_after = Some(Duration::from_millis(60));
    let inhibited = h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        let (surface, _toplevel) = c.open_window();
        c.attach_buffer(&surface, 400, 300);
        let (globals, q2) = wayland_client::globals::registry_queue_init::<App>(&conn).unwrap();
        let manager: ZwpIdleInhibitManagerV1 = globals.bind(&q2.handle(), 1..=1, ()).expect("idle inhibit manager");
        // A video playing: keep the screen on.
        let _inhibitor = manager.create_inhibitor(&surface, &c.queue.handle(), ());
        c.queue.roundtrip(&mut c.app).unwrap();
        ask.run(|st| st.activity());
        std::thread::sleep(Duration::from_millis(150));
        let held = ask.run(|st| (st.idle.locked, st.idle.screens_off, st.idle.inhibitors.len()));
        drop(q2);
        held
    });
    assert_eq!(inhibited, (false, false, 1), "neither lock nor screens off while the window keeps it awake");
}

#[test]
fn nested_never_idles() {
    let mut h = Headless::new(true);
    h.state.idle.enabled = false;
    h.state.idle.lock_after = Some(Duration::from_millis(20));
    h.state.idle.off_after = Some(Duration::from_millis(20));
    h.state.activity();
    run_for(&mut h, Duration::from_millis(60));
    assert!(!h.state.idle.locked && !h.state.idle.screens_off);
}
