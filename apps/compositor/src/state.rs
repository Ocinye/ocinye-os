//! What the compositor holds: the Wayland globals, the one space the windows
//! are mapped on, and the seat.

use std::ffi::OsString;
use std::sync::Arc;

use smithay::desktop::{PopupManager, Space, Window, WindowSurfaceType};
use smithay::input::{Seat, SeatState};
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{EventLoop, Interest, LoopHandle, LoopSignal, Mode, PostAction};
use smithay::reexports::wayland_server::backend::{ClientData, ClientId, DisconnectReason};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Display, DisplayHandle};
use smithay::utils::{Logical, Point};
use smithay::wayland::compositor::{CompositorClientState, CompositorState};
use smithay::wayland::output::OutputManagerState;
use smithay::wayland::selection::data_device::DataDeviceState;
use smithay::wayland::shell::xdg::XdgShellState;
use smithay::wayland::shm::ShmState;
use smithay::wayland::socket::ListeningSocketSource;

use crate::drm::Backend;

/// The compositor.
pub struct State {
    /// When it started: the clock frame callbacks are stamped with.
    pub start_time: std::time::Instant,
    /// When `/etc/default/keyboard` last changed, as last applied.
    pub keymap_seen: Option<std::time::SystemTime>,
    /// The Wayland socket's name.
    pub socket_name: OsString,
    /// The display.
    pub display_handle: DisplayHandle,
    /// The event loop.
    pub handle: LoopHandle<'static, State>,
    /// Stops the event loop.
    pub loop_signal: LoopSignal,

    /// The plane the windows are mapped on.
    pub space: Space<Window>,
    /// Popups (menus, tooltips) of the mapped windows.
    pub popups: PopupManager,
    /// Where the pointer is.
    pub pointer_location: Point<f64, Logical>,

    /// `wl_compositor`.
    pub compositor_state: CompositorState,
    /// `xdg_wm_base`.
    pub xdg_shell_state: XdgShellState,
    /// `wl_shm`.
    pub shm_state: ShmState,
    /// `wl_output` and `xdg_output`.
    pub output_manager_state: OutputManagerState,
    /// `wl_seat`.
    pub seat_state: SeatState<State>,
    /// `wl_data_device_manager`.
    pub data_device_state: DataDeviceState,
    /// The one seat.
    pub seat: Seat<Self>,

    /// The screen, once taken.
    pub backend: Option<Backend>,
    /// The interface process.
    pub client: Option<std::process::Child>,

    /// The trusted system surface: the window of the process this compositor
    /// started itself. Identified by the kernel's word about the client, never
    /// by anything the client says about itself.
    pub trusted: Option<Window>,
    /// Whether the trusted surface asked for the whole screen.
    pub trusted_full: bool,
    /// Every other window, oldest first. Only the newest is shown.
    pub apps: Vec<Window>,
    /// Whether the Session Service says a session is active. Without a yes,
    /// only the trusted surface is on the screen.
    pub session_active: bool,
    /// Idle seconds after which the session must be locked; zero for never.
    pub idle_lock_secs: u32,
    /// When the person last touched the keyboard or the pointer.
    pub last_input: std::time::Instant,
}

impl State {
    /// Apply the machine's keyboard layout again when its file changed.
    pub fn follow_keymap(&mut self) {
        let (keymap, seen) = crate::keymap::read(std::path::Path::new(crate::keymap::FILE));
        if seen == self.keymap_seen {
            return;
        }
        self.keymap_seen = seen;
        if let Some(keyboard) = self.seat.get_keyboard() {
            if keyboard.set_xkb_config(self, keymap.config()).is_err() {
                eprintln!("ocinye-compositor: the keyboard layout in {} does not compile; kept the previous one", crate::keymap::FILE);
            }
        }
    }

    /// Create the globals and start listening.
    pub fn new(event_loop: &mut EventLoop<'static, State>, display: Display<Self>) -> Self {
        let dh = display.handle();
        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let mut seat_state = SeatState::new();
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        let mut seat: Seat<Self> = seat_state.new_wl_seat(&dh, "seat0");
        // The machine's layout; a file that does not compile falls back to
        // xkbcommon's default rather than leaving the screen without a keyboard.
        let (keymap, keymap_seen) = crate::keymap::read(std::path::Path::new(crate::keymap::FILE));
        if seat.add_keyboard(keymap.config(), 300, 30).is_err() {
            seat.add_keyboard(Default::default(), 300, 30)
                .expect("the default keymap compiles");
        }
        seat.add_pointer();

        let socket_name = Self::listen(display, event_loop);
        Self {
            start_time: std::time::Instant::now(),
            keymap_seen,
            socket_name,
            display_handle: dh,
            handle: event_loop.handle(),
            loop_signal: event_loop.get_signal(),
            space: Space::default(),
            popups: PopupManager::default(),
            pointer_location: (0.0, 0.0).into(),
            compositor_state,
            xdg_shell_state,
            shm_state,
            output_manager_state,
            seat_state,
            data_device_state,
            seat,
            backend: None,
            client: None,
            trusted: None,
            trusted_full: true,
            apps: Vec::new(),
            session_active: false,
            idle_lock_secs: 0,
            last_input: std::time::Instant::now(),
        }
    }

    fn listen(display: Display<State>, event_loop: &mut EventLoop<'static, State>) -> OsString {
        let socket = ListeningSocketSource::new_auto().expect("a Wayland socket can be created");
        let name = socket.socket_name().to_os_string();
        let handle = event_loop.handle();
        handle
            .insert_source(socket, move |stream, (), state| {
                let _ = state
                    .display_handle
                    .insert_client(stream, Arc::new(ClientState::default()));
            })
            .expect("the socket source registers");
        handle
            .insert_source(
                Generic::new(display, Interest::READ, Mode::Level),
                |_, display, state| {
                    // SAFETY: the display is owned by this source and never dropped
                    // while the loop runs.
                    unsafe {
                        let _ = display.get_mut().dispatch_clients(state);
                    }
                    Ok(PostAction::Continue)
                },
            )
            .expect("the display source registers");
        name
    }

    /// The surface under a point, with its origin.
    pub fn surface_under(
        &self,
        pos: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<f64, Logical>)> {
        self.space
            .element_under(pos)
            .and_then(|(window, location)| {
                window
                    .surface_under(pos - location.to_f64(), WindowSurfaceType::ALL)
                    .map(|(s, p)| (s, (p + location).to_f64()))
            })
    }
}

/// Per-client state.
#[derive(Default)]
pub struct ClientState {
    /// The client's compositor state.
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _: ClientId) {}
    fn disconnected(&self, _: ClientId, _: DisconnectReason) {}
}
