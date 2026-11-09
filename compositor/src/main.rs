//! whiterose-wm: the WhiteRose Wayland compositor.
//!
//! For now it only runs nested inside another desktop (a window on your
//! current session). Usage:
//!
//!   whiterose-wm [--mod super|ctrl-alt] [--shell COMMAND] [-c COMMAND]
//!
//! --mod  What stands in for SUPER. Defaults to ctrl-alt when nested, because
//!        the host desktop usually keeps SUPER for itself.
//! --shell  Start the shell (bar and panels), e.g. "qs -p shell".
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

    let mut main_mod = "ctrl-alt".to_string();
    let mut startup: Option<String> = None;
    let mut shell: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--mod" => main_mod = args.next().ok_or("--mod needs a value")?,
            "--shell" => shell = Some(args.next().ok_or("--shell needs a command")?),
            "-c" | "--command" => startup = Some(args.next().ok_or("-c needs a command")?),
            "-h" | "--help" => {
                println!("usage: whiterose-wm [--mod super|ctrl-alt] [--shell COMMAND] [-c COMMAND]");
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

    if let Some(cmd) = shell {
        state.spawn(&cmd);
    }
    if let Some(cmd) = startup {
        state.spawn(&cmd);
    }

    event_loop.run(None, &mut state, |_| {})?;
    Ok(())
}
