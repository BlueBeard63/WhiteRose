//! whiterose-wm: the WhiteRose Wayland compositor.
//!
//! For now it only runs nested inside another desktop (a window on your
//! current session). Usage:
//!
//!   whiterose-wm [--mod super|ctrl-alt] [--daemon COMMAND] [--shell COMMAND] [-c COMMAND]
//!
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
    let mut startup: Option<String> = None;
    let mut shell: Option<String> = None;
    let mut daemon: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--mod" => main_mod = args.next().ok_or("--mod needs a value")?,
            "--shell" => shell = Some(args.next().ok_or("--shell needs a command")?),
            "--daemon" => daemon = Some(args.next().ok_or("--daemon needs a command")?),
            "-c" | "--command" => startup = Some(args.next().ok_or("-c needs a command")?),
            "-h" | "--help" => {
                println!(
                    "usage: whiterose-wm [--mod super|ctrl-alt] [--daemon COMMAND] [--shell COMMAND] [-c COMMAND]"
                );
                return Ok(());
            }
            other => return Err(format!("unknown argument {other:?}").into()),
        }
    }

    let terminal = std::env::var("TERMINAL").unwrap_or_else(|_| "ptyxis --standalone".into());
    let bindings = match main_mod.as_str() {
        "super" => Bindings::defaults(&terminal),
        "ctrl-alt" => Bindings::defaults(&terminal).remap_logo(Mods { ctrl: true, alt: true, ..Mods::NONE }),
        other => return Err(format!("--mod must be super or ctrl-alt, not {other:?}").into()),
    };

    let mut event_loop: EventLoop<'static, WhiteRose> = EventLoop::try_new()?;
    let display: Display<WhiteRose> = Display::new()?;
    let mut state = WhiteRose::new(&mut event_loop, display, bindings);

    whiterose_wm::winit::init_winit(&mut event_loop, &mut state)?;
    tracing::info!(socket = ?state.socket_name, main_mod, "WhiteRose is running");

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

    event_loop.run(None, &mut state, |state| state.after_dispatch())?;
    Ok(())
}
