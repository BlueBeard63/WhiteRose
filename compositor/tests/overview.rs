//! The overview: SUPER+W spreads the current workspace's windows (none
//! overlapping) under a row of every workspace; the pointer goes to it,
//! not the apps. A click on a window goes to it, on a workspace goes
//! there, and a window dragged onto a workspace moves there. Three fingers
//! up open it following the fingers, down close it; Esc closes it; with
//! `overview.style` off it doesn't open.

mod common;

use common::{tiles::*, *};
use serde_json::json;
use smithay::{
    backend::input::ButtonState,
    utils::{Logical, Point},
};
use whiterose_wm::{keybind::Action, overview::Rect, WhiteRose};

const LEFT: u32 = 0x110;

fn middle(s: &WhiteRose, r: &Rect) -> Point<f64, Logical> {
    let o = s.space.outputs().next().unwrap();
    let at = s.space.output_geometry(o).unwrap().loc.to_f64();
    at + Point::from((r.x + r.w / 2.0, r.y + r.h / 2.0))
}

fn click(s: &mut WhiteRose, at: Point<f64, Logical>) {
    s.on_pointer_motion(at, 1);
    s.on_pointer_button(LEFT, ButtonState::Pressed, 2);
    s.on_pointer_button(LEFT, ButtonState::Released, 3);
}

fn two_windows(c: &mut Client) {
    for _ in 0..2 {
        let (a, _ta) = open(c);
        c.attach_buffer(&a, 600, 400);
    }
    c.queue.roundtrip(&mut c.app).unwrap();
}

#[test]
fn super_w_spreads_the_windows_and_a_click_goes_to_one() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        two_windows(&mut c);
        ask.run(|s| {
            s.run_action(Action::Overview(None));
            assert!(s.overview_active());
            let o = s.space.outputs().next().cloned().unwrap();
            let scene = s.overview_scene(&o).expect("a scene");
            assert_eq!(scene.windows.len(), 2);
            let (a, b) = (scene.windows[0].2, scene.windows[1].2);
            assert!(a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y, "apart");
            assert_eq!(scene.cells.len(), 9);
            let view = s.overview_view().expect("the shell is told");
            assert_eq!(view.outputs[0].windows.len(), 2);

            // The pointer outlines a window, and the app under it doesn't get it.
            let first = scene.windows[0].0.clone();
            s.on_pointer_motion(middle(s, &a), 1);
            assert_eq!(s.overview.hovered.as_ref(), Some(&first));
            assert!(s.seat.get_pointer().unwrap().current_focus().is_none());

            click(s, middle(s, &a));
            assert_eq!(s.overview.open.target(), 0.0, "closing");
            assert_eq!(s.focused_window().as_ref(), Some(&first));

            // Esc closes it too.
            s.run_action(Action::Overview(Some(true)));
            s.run_action(Action::Overview(Some(false)));
            assert_eq!(s.overview.open.target(), 0.0);
        });
    });
}

#[test]
fn workspaces_in_the_row_take_clicks_and_dropped_windows() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        two_windows(&mut c);
        ask.run(|s| {
            s.run_action(Action::Overview(None));
            let o = s.space.outputs().next().cloned().unwrap();
            let scene = s.overview_scene(&o).unwrap();
            let (w, _, r) = scene.windows[0].clone();
            // Dragged onto workspace 4: it moves there, the overview stays.
            s.on_pointer_motion(middle(s, &r), 1);
            s.on_pointer_button(LEFT, ButtonState::Pressed, 2);
            s.on_pointer_motion(middle(s, &r) + Point::from((30.0, 0.0)), 3);
            assert!(s.overview.dragging.is_some());
            s.on_pointer_motion(middle(s, &scene.cells[3]), 4);
            s.on_pointer_button(LEFT, ButtonState::Released, 5);
            assert_eq!(s.workspaces.workspace_of(&w), Some(3));
            assert!(s.overview_active());
            assert_eq!(s.overview_scene(&o).unwrap().windows.len(), 1);
            // A click on workspace 4 goes there; the overview stays.
            click(s, middle(s, &scene.cells[3]));
            assert_eq!(s.workspaces.active(), 3);
            assert!(s.overview_active());
            assert_eq!(s.overview_scene(&o).unwrap().windows.len(), 1);
            // A click on nothing closes it.
            click(s, Point::from((5.0, 800.0)));
            assert_eq!(s.overview.open.target(), 0.0);
        });
    });
}

#[test]
fn three_fingers_open_and_close_it_and_the_style_can_turn_it_off() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        ask.run(|s| {
            s.swipe_begin(3, 0);
            s.swipe_update(1, Point::from((0.0, -20.0)));
            s.swipe_update(2, Point::from((0.0, -105.0)));
            assert!(s.overview.open.driven());
            let half = s.overview_shown();
            assert!((half - 0.5).abs() < 1e-6, "{half}: following the fingers");
            s.swipe_end(3, false);
            assert_eq!(s.overview.open.target(), 1.0, "half way opens it");
            // Down a little and back: stays open.
            s.overview.open.jump(1.0);
            s.swipe_begin(3, 10);
            s.swipe_update(11, Point::from((0.0, 30.0)));
            std::thread::sleep(std::time::Duration::from_millis(100));
            s.swipe_end(12, false);
            assert_eq!(s.overview.open.target(), 1.0);
            // Down all the way closes it.
            s.swipe_begin(3, 20);
            s.swipe_update(21, Point::from((0.0, 300.0)));
            s.swipe_end(22, false);
            assert_eq!(s.overview.open.target(), 0.0);

            s.overview_setting("overview.style", &json!("off"));
            s.run_action(Action::Overview(None));
            assert!(!s.overview_active());
            s.swipe_begin(3, 30);
            s.swipe_update(31, Point::from((0.0, -200.0)));
            assert!(!s.overview_active(), "the gesture is the app's then");
            s.swipe_end(32, false);
        });
    });
}

/// Near the right edge of `r`, half way down (a neighbouring workspace
/// shows only its edge).
fn right_edge(s: &WhiteRose, r: &Rect) -> Point<f64, Logical> {
    middle(s, r) + Point::from((r.w / 2.0 - 20.0, 0.0))
}

#[test]
fn niri_style_zooms_out_on_the_workspaces_side_by_side() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        two_windows(&mut c);
        ask.run(|s| {
            s.overview_setting("overview.style", &json!("niri"));
            s.animation_setting("animations.workspaceSwitch", &json!(false));
            let o = s.space.outputs().next().cloned().unwrap();
            let geo = s.space.output_geometry(&o).unwrap();
            let second = s.workspaces.active_windows()[1].clone();
            s.move_window_to(&second, 1);
            s.run_action(Action::Overview(None));
            let scene = s.overview_scene(&o).unwrap();
            assert!(scene.zoom.is_some());
            assert_eq!(scene.windows.len(), 2, "every workspace's windows");
            assert_eq!(scene.cells.len(), 9);
            let cur = scene.cells[0];
            assert!((cur.w - geo.size.w as f64 / 2.0).abs() < 1e-6, "half size");
            assert!((cur.x + cur.w / 2.0 - geo.size.w as f64 / 2.0).abs() < 1e-6, "the current one in the middle");
            let (_, _, r) = scene.windows.iter().find(|(w, _, _)| *w == second).cloned().unwrap();
            let next = scene.cells[1];
            assert!(r.x >= next.x && r.x + r.w <= next.x + next.w + 1e-6, "on its own workspace, to the right");
            let view = s.overview_view().unwrap();
            assert_eq!(view.style, "niri");
            assert_eq!(view.search, None, "no search");
            assert!(view.outputs[0].windows.iter().all(|w| w.title_y.is_some()));

            // Scrolling moves along; the arrows' and a scroll's step stop at the ends.
            assert!(s.overview_scroll(15.0, false));
            assert_eq!(s.workspaces.active(), 1);
            assert!(s.overview_active());
            s.overview_scroll(-15.0, false);
            assert_eq!(s.workspaces.active(), 0);
            assert_eq!(s.overview_step_target(-1), None);

            // A window on the next workspace: a click goes to it.
            click(s, middle(s, &r));
            assert_eq!(s.workspaces.active(), 1);
            assert_eq!(s.focused_window().as_ref(), Some(&second));
            assert_eq!(s.overview.open.target(), 0.0, "closing");

            // Dragged onto the workspace before: it moves there, the overview stays.
            s.overview.open.jump(0.0);
            s.run_action(Action::Overview(None));
            let scene = s.overview_scene(&o).unwrap();
            let (_, _, r) = scene.windows.iter().find(|(w, _, _)| *w == second).cloned().unwrap();
            s.on_pointer_motion(middle(s, &r), 1);
            s.on_pointer_button(LEFT, ButtonState::Pressed, 2);
            s.on_pointer_motion(middle(s, &r) + Point::from((30.0, 0.0)), 3);
            assert!(s.overview.dragging.is_some());
            s.on_pointer_motion(right_edge(s, &scene.cells[0]), 4);
            s.on_pointer_button(LEFT, ButtonState::Released, 5);
            assert_eq!(s.workspaces.workspace_of(&second), Some(0));
            assert!(s.overview_active());

            // A click on a workspace's empty space goes there and closes it.
            let scene = s.overview_scene(&o).unwrap();
            click(s, right_edge(s, &scene.cells[0]));
            assert_eq!(s.workspaces.active(), 0);
            assert_eq!(s.overview.open.target(), 0.0);

            // GNOME's again: its own scene, its search.
            s.overview_setting("overview.style", &json!("gnome"));
            s.run_action(Action::Overview(None));
            assert!(s.overview_scene(&o).unwrap().zoom.is_none());
            assert!(s.overview_view().unwrap().search.is_some());
            assert!(!s.overview_scroll(15.0, false), "GNOME's doesn't scroll");
        });
    });
}

fn renderer() -> Option<smithay::backend::renderer::gles::GlesRenderer> {
    use smithay::backend::egl::{EGLContext, EGLDevice, EGLDisplay};
    let devices: Vec<EGLDevice> = EGLDevice::enumerate().ok()?.collect();
    let device = devices
        .iter()
        .find(|d| d.extensions().iter().any(|e| e == "EGL_MESA_device_software"))
        .or(devices.first())?
        .clone();
    let display = unsafe { EGLDisplay::new(device) }.ok()?;
    let context = EGLContext::new(&display).ok()?;
    unsafe { smithay::backend::renderer::gles::GlesRenderer::new(context) }.ok()
}

#[test]
fn spread_windows_are_drawn_at_their_spread_size() {
    // A surface element's size comes from the output's scale when it is
    // drawn, so a window built with a smaller `Scale` still came out full
    // size, past its outline (0.4.0-alpha.1).
    use smithay::backend::renderer::element::Element;
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        for _ in 0..2 {
            let (a, _ta) = open(&mut c);
            c.attach_buffer(&a, 1600, 1000);
        }
        c.queue.roundtrip(&mut c.app).unwrap();
        ask.run(|s| {
            let Some(mut r) = renderer() else {
                eprintln!("no EGL device: skipping");
                return;
            };
            s.run_action(Action::Overview(None));
            s.overview.open.jump(1.0);
            let o = s.space.outputs().next().cloned().unwrap();
            let scene = s.overview_scene(&o).expect("a scene");
            let out = whiterose_wm::overview::elements(&mut r, &o, s).expect("elements");
            let scale = smithay::utils::Scale::from(o.current_scale().fractional_scale());
            assert!(scene.windows.iter().all(|(w, _, to)| to.w < w.geometry().size.w as f64 * 0.9), "shrunk");
            for (_, _, to) in &scene.windows {
                let found = out.iter().map(|e| e.geometry(scale)).any(|g| {
                    (g.loc.x - to.x.round() as i32).abs() <= 1
                        && (g.loc.y - to.y.round() as i32).abs() <= 1
                        && (g.size.w - to.w.round() as i32).abs() <= 1
                        && (g.size.h - to.h.round() as i32).abs() <= 1
                });
                assert!(found, "nothing drawn at {to:?}");
            }
        });
    });
}
