//! The Wayland protocol handlers.
//!
//! The window policy is deliberately small. The trusted system surface has
//! the whole screen, or the bar along its top while a session is active; the
//! newest application window has the rest. Arranging application windows is
//! the Workspace's job, inside the surface it is given.

use smithay::backend::renderer::utils::on_commit_buffer_handler;
use smithay::desktop::{find_popup_root_surface, get_popup_toplevel_coords, PopupKind, Window};
use smithay::input::pointer::CursorImageStatus;
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_seat::WlSeat;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, Resource};
use smithay::utils::{Serial, SERIAL_COUNTER};
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{
    get_parent, is_sync_subsurface, with_states, CompositorClientState, CompositorHandler,
    CompositorState,
};
use smithay::wayland::output::OutputHandler;
use smithay::wayland::selection::data_device::{
    set_data_device_focus, ClientDndGrabHandler, DataDeviceHandler, DataDeviceState,
    ServerDndGrabHandler,
};
use smithay::wayland::selection::SelectionHandler;
use smithay::wayland::shell::xdg::{
    PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
    XdgToplevelSurfaceData,
};
use smithay::wayland::shm::{ShmHandler, ShmState};
use smithay::{
    delegate_compositor, delegate_data_device, delegate_output, delegate_seat, delegate_shm,
    delegate_xdg_shell,
};

use crate::state::ClientState;
use crate::State;

impl CompositorHandler for State {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client
            .get_data::<ClientState>()
            .expect("every client carries ClientState")
            .compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);
        if !is_sync_subsurface(surface) {
            let mut root = surface.clone();
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }
            if let Some(window) = self.window_for(&root) {
                window.on_commit();
            }
        }
        // First commit of a top-level: answer with the size it must take.
        if let Some(window) = self.window_for(surface) {
            let sent = with_states(surface, |states| {
                states
                    .data_map
                    .get::<XdgToplevelSurfaceData>()
                    .is_some_and(|d| d.lock().is_ok_and(|d| d.initial_configure_sent))
            });
            if !sent {
                if let Some(toplevel) = window.toplevel() {
                    toplevel.send_configure();
                }
            }
        }
        self.popups.commit(surface);
        if let Some(PopupKind::Xdg(popup)) = self.popups.find_popup(surface) {
            if !popup.is_initial_configure_sent() {
                let _ = popup.send_configure();
            }
        }
        self.request_redraw();
    }
}

impl BufferHandler for State {
    fn buffer_destroyed(&mut self, _: &WlBuffer) {}
}

impl ShmHandler for State {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

delegate_compositor!(State);
delegate_shm!(State);

impl State {
    fn window_for(&self, surface: &WlSurface) -> Option<Window> {
        self.trusted
            .iter()
            .chain(self.apps.iter())
            .find(|w| w.toplevel().is_some_and(|t| t.wl_surface() == surface))
            .cloned()
    }

    /// Whether a surface belongs to the process this compositor started.
    fn is_trusted_client(&self, surface: &WlSurface) -> bool {
        let Some(pid) = self.client.as_ref().map(std::process::Child::id) else {
            return false;
        };
        self.display_handle
            .get_client(surface.id())
            .ok()
            .and_then(|c| c.get_credentials(&self.display_handle).ok())
            .is_some_and(|c| u32::try_from(c.pid).is_ok_and(|p| p == pid))
    }

    /// Whether only the trusted surface may be on the screen now.
    pub fn exclusive(&self) -> bool {
        self.trusted_full || !self.session_active
    }

    /// Put every window where the policy says it goes.
    ///
    /// The trusted surface takes the whole screen when it asks for it and
    /// whenever no session is active; otherwise it is the bar along the top,
    /// and the newest application window takes the rest. Without a trusted
    /// surface nothing is shown at all: an application never has the screen
    /// to itself.
    pub fn arrange(&mut self) {
        let Some(size) = self
            .space
            .outputs()
            .next()
            .and_then(|o| self.space.output_geometry(o))
            .map(|g| g.size)
        else {
            return;
        };
        let bar = BAR_HEIGHT.min(size.h);
        let exclusive = self.exclusive();
        for app in &self.apps {
            self.space.unmap_elem(app);
        }
        let mut focus = None;
        if let Some(trusted) = self.trusted.clone() {
            if let Some(toplevel) = trusted.toplevel() {
                place(
                    toplevel,
                    if exclusive {
                        size
                    } else {
                        (size.w, bar).into()
                    },
                    if exclusive {
                        Placement::Screen
                    } else {
                        Placement::Bar
                    },
                );
                focus = Some(toplevel.wl_surface().clone());
            }
            self.space.map_element(trusted, (0, 0), true);
            if !exclusive {
                if let Some(app) = self.apps.last().cloned() {
                    if let Some(toplevel) = app.toplevel() {
                        place(toplevel, (size.w, size.h - bar).into(), Placement::Below);
                        focus = Some(toplevel.wl_surface().clone());
                    }
                    self.space.map_element(app, (0, bar), true);
                }
            }
        } else {
            let mapped: Vec<Window> = self.space.elements().cloned().collect();
            for window in mapped {
                self.space.unmap_elem(&window);
            }
        }
        if let Some(keyboard) = self.seat.get_keyboard() {
            keyboard.set_focus(self, focus, SERIAL_COUNTER.next_serial());
        }
        self.request_redraw();
    }
}

/// The height of the bar the trusted surface keeps while a session is active.
const BAR_HEIGHT: i32 = 44;

enum Placement {
    /// The whole screen.
    Screen,
    /// The bar along the top.
    Bar,
    /// The area below the bar.
    Below,
}

fn place(
    toplevel: &ToplevelSurface,
    size: smithay::utils::Size<i32, smithay::utils::Logical>,
    placement: Placement,
) {
    use xdg_toplevel::State as S;
    toplevel.with_pending_state(|state| {
        state.size = Some(size);
        for flag in [
            S::Fullscreen,
            S::Maximized,
            S::TiledTop,
            S::TiledBottom,
            S::TiledLeft,
            S::TiledRight,
        ] {
            state.states.unset(flag);
        }
        match placement {
            Placement::Screen => {
                state.states.set(S::Fullscreen);
            }
            Placement::Bar => {
                for flag in [S::TiledTop, S::TiledBottom, S::TiledLeft, S::TiledRight] {
                    state.states.set(flag);
                }
            }
            Placement::Below => {
                state.states.set(S::Maximized);
            }
        }
        state.states.set(S::Activated);
    });
    if toplevel.is_initial_configure_sent() {
        toplevel.send_pending_configure();
    }
}

impl XdgShellHandler for State {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let trusted = self.trusted.is_none() && self.is_trusted_client(surface.wl_surface());
        let window = Window::new_wayland_window(surface);
        if trusted {
            self.trusted = Some(window);
        } else {
            self.apps.push(window);
        }
        self.arrange();
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        let is = |w: &Window| {
            w.toplevel()
                .is_some_and(|t| t.wl_surface() == surface.wl_surface())
        };
        if self.trusted.as_ref().is_some_and(is) {
            self.trusted = None;
        }
        self.apps.retain(|w| !is(w));
        self.arrange();
    }

    // Only the trusted surface chooses between the whole screen and the bar.
    // An application's request changes nothing: it has the area it is given.
    fn fullscreen_request(&mut self, surface: ToplevelSurface, _: Option<WlOutput>) {
        if self
            .trusted
            .as_ref()
            .and_then(Window::toplevel)
            .is_some_and(|t| t.wl_surface() == surface.wl_surface())
        {
            self.trusted_full = true;
        }
        self.arrange();
        if surface.is_initial_configure_sent() {
            surface.send_configure();
        }
    }

    fn unfullscreen_request(&mut self, surface: ToplevelSurface) {
        if self
            .trusted
            .as_ref()
            .and_then(Window::toplevel)
            .is_some_and(|t| t.wl_surface() == surface.wl_surface())
        {
            self.trusted_full = false;
        }
        self.arrange();
    }

    fn new_popup(&mut self, surface: PopupSurface, _: PositionerState) {
        self.unconstrain_popup(&surface);
        let _ = self.popups.track_popup(PopupKind::Xdg(surface));
    }

    fn reposition_request(
        &mut self,
        surface: PopupSurface,
        positioner: PositionerState,
        token: u32,
    ) {
        surface.with_pending_state(|state| {
            state.geometry = positioner.get_geometry();
            state.positioner = positioner;
        });
        self.unconstrain_popup(&surface);
        surface.send_repositioned(token);
    }

    // A window cannot move, resize or grab: it has the area it is given.
    fn move_request(&mut self, _: ToplevelSurface, _: WlSeat, _: Serial) {}
    fn resize_request(
        &mut self,
        _: ToplevelSurface,
        _: WlSeat,
        _: Serial,
        _: xdg_toplevel::ResizeEdge,
    ) {
    }
    fn grab(&mut self, _: PopupSurface, _: WlSeat, _: Serial) {}
}

delegate_xdg_shell!(State);

impl State {
    fn unconstrain_popup(&self, popup: &PopupSurface) {
        let Ok(root) = find_popup_root_surface(&PopupKind::Xdg(popup.clone())) else {
            return;
        };
        let Some(window) = self.window_for(&root) else {
            return;
        };
        let Some(output) = self.space.outputs().next() else {
            return;
        };
        let (Some(output_geo), Some(window_geo)) = (
            self.space.output_geometry(output),
            self.space.element_geometry(&window),
        ) else {
            return;
        };
        let mut target = output_geo;
        target.loc -= get_popup_toplevel_coords(&PopupKind::Xdg(popup.clone()));
        target.loc -= window_geo.loc;
        popup.with_pending_state(|state| {
            state.geometry = state.positioner.get_unconstrained_geometry(target);
        });
    }
}

impl SeatHandler for State {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<State> {
        &mut self.seat_state
    }

    fn cursor_image(&mut self, _: &Seat<Self>, _: CursorImageStatus) {}

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        let client = focused.and_then(|s| self.display_handle.get_client(s.id()).ok());
        set_data_device_focus(&self.display_handle, seat, client);
    }
}

delegate_seat!(State);

impl SelectionHandler for State {
    type SelectionUserData = ();
}

impl DataDeviceHandler for State {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}

impl ClientDndGrabHandler for State {}
impl ServerDndGrabHandler for State {}

delegate_data_device!(State);

impl OutputHandler for State {}
delegate_output!(State);
