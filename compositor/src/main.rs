//! whiterose-wm: the WhiteRose Wayland compositor.
//!
//! Runs on the hardware when started from a text console (or a display
//! manager), or as a window inside another desktop for development. Usage:
//!
//!   whiterose-wm [--backend auto|drm|nested] [--mod super|ctrl-alt] [--daemon COMMAND] [--shell COMMAND] [-c COMMAND]
//!
//! --backend  drm: the real screens and input (needs a logind session, e.g.
//!            a console login). nested: a window in the current desktop.
//!            auto (the default): nested inside a desktop, drm otherwise.
//! --mod  What stands in for SUPER. Defaults to super: the nested window asks
//!        the host desktop to pass its shortcuts through. Use ctrl-alt if the
//!        host can't (X11, or the permission was denied).
//! --shell  Start the shell (bar and panels), e.g. "qs -p shell". It is
//!          restarted if it exits.
//! --daemon Start whiterosed, e.g. "whiterosed serve" (settings, system
//!          state); also restarted if it exits.
//! -c       Run COMMAND inside the compositor at startup.

use smithay::reexports::{calloop::EventLoop, wayland_server::Display};
use whiterose_wm::{
    keybind::{Bindings, Mods},
    WhiteRose,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // The nested window asks the host to pass its shortcuts through, so the
    // real SUPER bindings work there too; ctrl-alt remains as a fallback.
    let mut main_mod = "super".to_string();
    let mut backend = "auto".to_string();
    let mut startup: Option<String> = None;
    let mut shell: Option<String> = None;
    let mut daemon: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--backend" => backend = args.next().ok_or("--backend needs a value")?,
            "--mod" => main_mod = args.next().ok_or("--mod needs a value")?,
            "--shell" => shell = Some(args.next().ok_or("--shell needs a command")?),
            "--daemon" => daemon = Some(args.next().ok_or("--daemon needs a command")?),
            "-c" | "--command" => startup = Some(args.next().ok_or("-c needs a command")?),
            "-h" | "--help" => {
                println!(
                    "usage: whiterose-wm [--backend auto|drm|nested] [--mod super|ctrl-alt] [--daemon COMMAND] [--shell COMMAND] [-c COMMAND]"
                );
                return Ok(());
            }
            other => return Err(format!("unknown argument {other:?}").into()),
        }
    }

    let terminal = std::env::var("TERMINAL").unwrap_or_else(|_| "ptyxis --standalone".into());
    let logo_remap = match main_mod.as_str() {
        "super" => None,
        "ctrl-alt" => Some(Mods { ctrl: true, alt: true, ..Mods::NONE }),
        other => return Err(format!("--mod must be super or ctrl-alt, not {other:?}").into()),
    };
    let bindings = Bindings::defaults(&terminal);

    let mut event_loop: EventLoop<'static, WhiteRose> = EventLoop::try_new()?;
    let display: Display<WhiteRose> = Display::new()?;
    let mut state = WhiteRose::new(&mut event_loop, display, bindings);
    // Shortcuts and keyboard layout: defaults and the system's layout now;
    // the user's settings arrive from whiterosed.
    state.keyboard.terminal = terminal;
    state.keyboard.logo_remap = logo_remap;
    state.apply_shortcuts();
    state.apply_keyboard();
    state.arm_idle();

    let nested = match backend.as_str() {
        "auto" => std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some(),
        "nested" | "winit" => true,
        "drm" => false,
        other => return Err(format!("--backend must be auto, drm or nested, not {other:?}").into()),
    };
    if nested {
        whiterose_wm::winit::init_winit(&mut event_loop, &mut state)?;
    } else {
        whiterose_wm::drm::init_drm(&mut event_loop, &mut state)?;
    }
    tracing::info!(socket = ?state.socket_name, main_mod, nested, "WhiteRose is running");

    // Display settings and the rest come from whiterosed (retried until it's up).
    state.connect_daemon();
    // The daemon first: the shell connects to it (and retries until it's up).
    if let Some(cmd) = daemon {
        state.supervise("daemon", cmd);
    }
    if let Some(cmd) = shell {
        state.supervise("shell", cmd);
    }
    if let Some(cmd) = startup {
        state.spawn(&cmd);
    }

    // Health check (whiterose-launch): the compositor is up.
    whiterose_wm::ready::mark("compositor");
    event_loop.run(None, &mut state, |state| state.after_dispatch())?;
    if state.exit_code != 0 {
        // The daemon and shell get SIGTERM as we go (PDEATHSIG).
        std::process::exit(state.exit_code);
    }
    Ok(())
}
