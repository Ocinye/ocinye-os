//! The Wayland protocol handlers.
//!
//! The window policy is deliberately small: every top-level window fills the
//! output. Arranging application windows is the Workspace's job, inside the
//! surface it is given; this compositor only shows system surfaces.

use smithay::backend::renderer::utils::on_commit_buffer_handler;
use smithay::desktop::{find_popup_root_surface, get_popup_toplevel_coords, PopupKind, Window};
use smithay::input::pointer::CursorImageStatus;
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_seat::WlSeat;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, Resource};
use smithay::utils::{Serial, SERIAL_COUNTER};
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{
    get_parent, is_sync_subsurface, with_states, CompositorClientState, CompositorHandler, CompositorState,
};
use smithay::wayland::output::OutputHandler;
use smithay::wayland::selection::data_device::{
    set_data_device_focus, ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
};
use smithay::wayland::selection::SelectionHandler;
use smithay::wayland::shell::xdg::{
    PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState, XdgToplevelSurfaceData,
};
use smithay::wayland::shm::{ShmHandler, ShmState};
use smithay::{delegate_compositor, delegate_data_device, delegate_output, delegate_seat, delegate_shm, delegate_xdg_shell};

use crate::state::ClientState;
use crate::State;

impl CompositorHandler for State {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client.get_data::<ClientState>().expect("every client carries ClientState").compositor_state
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
        self.space
            .elements()
            .find(|w| w.toplevel().is_some_and(|t| t.wl_surface() == surface))
            .cloned()
    }

    /// Make a top-level fill the output.
    fn fill_output(&self, toplevel: &ToplevelSurface) {
        let Some(size) = self.space.outputs().next().and_then(|o| self.space.output_geometry(o)).map(|g| g.size) else {
            return;
        };
        toplevel.with_pending_state(|state| {
            state.size = Some(size);
            state.states.set(xdg_toplevel::State::Fullscreen);
            state.states.set(xdg_toplevel::State::Activated);
        });
    }
}

impl XdgShellHandler for State {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        self.fill_output(&surface);
        let wl_surface = surface.wl_surface().clone();
        let window = Window::new_wayland_window(surface);
        self.space.map_element(window, (0, 0), true);
        // The newest window has the keyboard: there is one interface, and its
        // newest surface is the one it wants the person to use.
        if let Some(keyboard) = self.seat.get_keyboard() {
            keyboard.set_focus(self, Some(wl_surface), SERIAL_COUNTER.next_serial());
        }
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        // Focus falls back to the window now on top.
        let remaining = self
            .space
            .elements()
            .rev()
            .filter_map(|w| w.toplevel().map(|t| t.wl_surface().clone()))
            .find(|s| s != surface.wl_surface());
        if let Some(keyboard) = self.seat.get_keyboard() {
            keyboard.set_focus(self, remaining, SERIAL_COUNTER.next_serial());
        }
        self.request_redraw();
    }

    fn new_popup(&mut self, surface: PopupSurface, _: PositionerState) {
        self.unconstrain_popup(&surface);
        let _ = self.popups.track_popup(PopupKind::Xdg(surface));
    }

    fn reposition_request(&mut self, surface: PopupSurface, positioner: PositionerState, token: u32) {
        surface.with_pending_state(|state| {
            state.geometry = positioner.get_geometry();
            state.positioner = positioner;
        });
        self.unconstrain_popup(&surface);
        surface.send_repositioned(token);
    }

    // A window cannot move, resize or grab: it fills the output.
    fn move_request(&mut self, _: ToplevelSurface, _: WlSeat, _: Serial) {}
    fn resize_request(&mut self, _: ToplevelSurface, _: WlSeat, _: Serial, _: xdg_toplevel::ResizeEdge) {}
    fn grab(&mut self, _: PopupSurface, _: WlSeat, _: Serial) {}
}

delegate_xdg_shell!(State);

impl State {
    fn unconstrain_popup(&self, popup: &PopupSurface) {
        let Ok(root) = find_popup_root_surface(&PopupKind::Xdg(popup.clone())) else { return };
        let Some(window) = self.window_for(&root) else { return };
        let Some(output) = self.space.outputs().next() else { return };
        let (Some(output_geo), Some(window_geo)) = (self.space.output_geometry(output), self.space.element_geometry(&window)) else {
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
