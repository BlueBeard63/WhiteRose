//! Outputs and their configuration: what each one is (stable ID, make,
//! modes), applying the display settings to them, the trial-and-revert of a
//! layout being tested, and telling the shell (the Displays page) about them.

use std::{cell::RefCell, time::Duration};

use serde::{Deserialize, Serialize};
use smithay::{
    desktop::layer_map_for_output,
    output::{Output, Scale},
    reexports::calloop::timer::{TimeoutAction, Timer},
    utils::Transform,
};

use crate::{
    daemon_client::now_ms,
    display_config::{self, Connected, ModeInfo},
    ipc,
    state::WhiteRose,
};

/// What we know about an output, kept in its user data.
#[derive(Debug, Clone, Default)]
pub struct OutputInfo {
    pub id: String,
    pub strong: bool,
    pub connector: String,
    pub make: String,
    pub model: String,
    pub builtin: bool,
    pub physical_mm: (i32, i32),
    pub modes: Vec<ModeInfo>,
    /// The mode the settings ask for; the backend switches to it.
    pub wanted_mode: Option<ModeInfo>,
    pub enabled: bool,
    pub vrr: bool,
}

pub fn info(o: &Output) -> OutputInfo {
    o.user_data().get::<RefCell<OutputInfo>>().map(|i| i.borrow().clone()).unwrap_or_default()
}

pub fn set_info(o: &Output, info: OutputInfo) {
    o.user_data().insert_if_missing(|| RefCell::new(OutputInfo::default()));
    *o.user_data().get::<RefCell<OutputInfo>>().unwrap().borrow_mut() = info;
}

pub fn transform_from(s: &str) -> Transform {
    match s {
        "90" => Transform::_90,
        "180" => Transform::_180,
        "270" => Transform::_270,
        "flipped" => Transform::Flipped,
        "flipped-90" => Transform::Flipped90,
        "flipped-180" => Transform::Flipped180,
        "flipped-270" => Transform::Flipped270,
        _ => Transform::Normal,
    }
}

pub fn transform_name(t: Transform) -> &'static str {
    match t {
        Transform::_90 => "90",
        Transform::_180 => "180",
        Transform::_270 => "270",
        Transform::Flipped => "flipped",
        Transform::Flipped90 => "flipped-90",
        Transform::Flipped180 => "flipped-180",
        Transform::Flipped270 => "flipped-270",
        Transform::Normal => "normal",
    }
}

/// What this backend can change (the Displays page greys out the rest).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    pub modes: bool,
    pub rotation: bool,
    pub position: bool,
    pub disable: bool,
    pub vrr: bool,
    /// Screenshots can include the pointer (the compositor draws it).
    #[serde(default)]
    pub pointer: bool,
}

/// An output as the shell sees it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputView {
    pub id: String,
    pub strong: bool,
    pub connector: String,
    pub make: String,
    pub model: String,
    pub builtin: bool,
    pub physical_mm: (i32, i32),
    pub modes: Vec<ModeInfo>,
    pub mode: Option<ModeInfo>,
    pub scale: f64,
    pub transform: String,
    pub position: (i32, i32),
    pub enabled: bool,
    pub vrr: bool,
    pub profile: Option<String>,
}

impl WhiteRose {
    /// The display settings in force: a trial layout until its time is up,
    /// otherwise the saved one.
    pub fn effective_displays(&self) -> &display_config::Displays {
        match &self.display_trial {
            Some(t) if t.until > now_ms() => &t.config,
            _ => &self.displays,
        }
    }

    /// Applies the display settings to every connected output.
    pub fn apply_displays(&mut self) {
        let mut outputs = self.outputs.clone();
        // Built-in panel first, then by connector: the left-to-right order
        // when no profile places them.
        outputs.sort_by_key(|o| {
            let i = info(o);
            (!i.builtin, i.connector)
        });
        let connected: Vec<Connected> =
            outputs.iter().map(|o| Connected { id: info(o).id, modes: info(o).modes }).collect();
        let plans = display_config::plan(self.effective_displays(), &connected);
        for (o, p) in outputs.iter().zip(&plans) {
            let mut i = info(o);
            i.enabled = p.enabled;
            i.vrr = p.vrr;
            i.wanted_mode = if self.caps.modes { p.mode } else { None };
            set_info(o, i);
            let transform = if self.caps.rotation { Some(transform_from(&p.transform)) } else { None };
            let position = if self.caps.position { p.position } else { (0, 0) };
            o.change_current_state(None, transform, Some(Scale::Fractional(p.scale)), Some(position.into()));
            if p.enabled {
                self.space.map_output(o, position);
            } else {
                self.space.unmap_output(o);
            }
            layer_map_for_output(o).arrange();
        }
        self.display_profile = plans.iter().find_map(|p| p.profile.clone());
        self.refit_windows();
        self.island_sync();
        self.outputs_sync();
        self.arm_trial_timer();
        self.schedule_redraw();
    }

    /// A trial layout reverts when its time is up (even if Settings crashed).
    fn arm_trial_timer(&mut self) {
        let Some(t) = self.display_trial.clone() else { return };
        let left = t.until.saturating_sub(now_ms());
        if left == 0 {
            self.end_trial(t.until);
            return;
        }
        let _ =
            self.loop_handle.insert_source(Timer::from_duration(Duration::from_millis(left)), move |_, _, state| {
                if state.display_trial.as_ref().is_some_and(|cur| cur.until == t.until) {
                    state.end_trial(t.until);
                }
                TimeoutAction::Drop
            });
    }

    fn end_trial(&mut self, until: u64) {
        tracing::info!("display trial over: back to the saved layout");
        if self.display_trial.as_ref().is_some_and(|t| t.until == until) {
            self.display_trial = None;
            self.daemon_request(serde_json::json!({"op": "reset", "key": "displays.test"}));
            self.apply_displays();
        }
    }

    pub fn outputs_view(&self) -> Vec<OutputView> {
        let mut v: Vec<OutputView> = self
            .outputs
            .iter()
            .map(|o| {
                let i = info(o);
                OutputView {
                    id: i.id,
                    strong: i.strong,
                    connector: i.connector,
                    make: i.make,
                    model: i.model,
                    builtin: i.builtin,
                    physical_mm: i.physical_mm,
                    modes: i.modes,
                    mode: o.current_mode().map(|m| ModeInfo {
                        width: m.size.w,
                        height: m.size.h,
                        refresh: m.refresh,
                        preferred: o.preferred_mode() == Some(m),
                    }),
                    scale: o.current_scale().fractional_scale(),
                    transform: if self.caps.rotation {
                        transform_name(o.current_transform()).into()
                    } else {
                        "normal".into()
                    },
                    position: o.current_location().into(),
                    enabled: i.enabled,
                    vrr: i.vrr,
                    profile: self.display_profile.clone(),
                }
            })
            .collect();
        v.sort_by(|a, b| (!a.builtin, &a.connector).cmp(&(!b.builtin, &b.connector)));
        v
    }

    pub fn outputs_event(&self) -> ipc::Event {
        ipc::Event::Outputs {
            outputs: self.outputs_view(),
            capabilities: self.caps,
            trial_until: self.display_trial.as_ref().map(|t| t.until),
        }
    }

    pub fn outputs_sync(&mut self) {
        let e = self.outputs_event();
        if let Some(ipc) = self.ipc.as_mut() {
            ipc.update(e);
        }
    }
}
