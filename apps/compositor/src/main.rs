//! `ocinye-compositor`: the display server of an Ocinye OS seat.
//!
//!   ocinye-compositor -- PROGRAM [ARGS…]
//!
//! It owns the screen and the input devices of one seat. The program it
//! starts — the native system interface — is the trusted surface: it has the
//! whole screen, or the bar along the top while a session is active, and any
//! other window has the rest. It is not a desktop: there is no panel, no
//! launcher and no window list here. The Workspace's own window manager
//! manages the Workspace's windows, inside the surface this compositor shows.
//!
//! The program to run is fixed by the unit that starts the compositor. Nothing
//! at run time can ask it to start another.

mod drm;
mod handlers;
mod input;
mod session;
mod state;

use std::process::ExitCode;

use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_server::Display;

pub use state::State;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some("--") {
        eprintln!("usage: ocinye-compositor -- PROGRAM [ARGS…]");
        return ExitCode::from(64);
    }
    let Some(program) = args.next() else {
        eprintln!("usage: ocinye-compositor -- PROGRAM [ARGS…]");
        return ExitCode::from(64);
    };
    let program_args: Vec<String> = args.collect();

    let mut event_loop: EventLoop<State> = match EventLoop::try_new() {
        Ok(l) => l,
        Err(e) => {
            eprintln!("ocinye-compositor: event loop: {e}");
            return ExitCode::FAILURE;
        }
    };
    let display: Display<State> = match Display::new() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("ocinye-compositor: display: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut state = State::new(&mut event_loop, display);
    if let Err(e) = drm::init(&mut event_loop, &mut state) {
        eprintln!("ocinye-compositor: cannot take the screen: {e}");
        return ExitCode::FAILURE;
    }
    println!(
        "OCINYE-COMPOSITOR-READY {}",
        state.socket_name.to_string_lossy()
    );

    match std::process::Command::new(&program)
        .args(&program_args)
        .env("WAYLAND_DISPLAY", &state.socket_name)
        .env("XDG_SESSION_TYPE", "wayland")
        .env("GDK_BACKEND", "wayland")
        .spawn()
    {
        Ok(child) => state.client = Some(child),
        Err(e) => {
            eprintln!(
                "ocinye-compositor: cannot start the interface: {}",
                e.kind()
            );
            return ExitCode::FAILURE;
        }
    }

    if let Err(e) = event_loop.run(None, &mut state, |state| {
        // The interface is the only reason this compositor runs: when it
        // exits, so does the compositor, and the unit decides what follows.
        if let Some(child) = state.client.as_mut() {
            if matches!(child.try_wait(), Ok(Some(_))) {
                state.loop_signal.stop();
            }
        }
        // What was queued for clients during this turn of the loop goes out
        // now, whether or not a frame was drawn.
        let _ = state.display_handle.flush_clients();
    }) {
        eprintln!("ocinye-compositor: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
