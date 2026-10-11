//! The screen: one GPU, one connected output, the kernel's mode setting.
//!
//! The compositor asks the seat manager (`libseat`) for the session, opens the
//! primary GPU through it, drives the first connected connector at its
//! preferred mode, and reads input through `libinput`. Rendering is OpenGL ES
//! on GBM, which Mesa also provides in software: a machine or a virtual machine
//! without an accelerated GPU still gets a screen.
//!
//! One output in v1. A second connected display is not driven yet.

use std::time::Duration;

use smithay::backend::allocator::gbm::{GbmAllocator, GbmBufferFlags, GbmDevice};
use smithay::backend::allocator::Fourcc;
use smithay::backend::drm::compositor::{DrmCompositor, FrameFlags};
use smithay::backend::drm::exporter::gbm::GbmFramebufferExporter;
use smithay::backend::drm::{DrmDevice, DrmDeviceFd, DrmEvent};
use smithay::backend::egl::{EGLContext, EGLDisplay};
use smithay::backend::input::InputEvent;
use smithay::backend::libinput::{LibinputInputBackend, LibinputSessionInterface};
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::ImportMemWl;
use smithay::backend::session::libseat::LibSeatSession;
use smithay::backend::session::{Event as SessionEvent, Session};
use smithay::backend::udev::{all_gpus, primary_gpu};
use smithay::desktop::space::{space_render_elements, SpaceRenderElements};
use smithay::output::{Mode as WlMode, Output, PhysicalProperties, Subpixel};
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::calloop::EventLoop;
use smithay::reexports::drm::control::{connector, Device as ControlDevice, ModeTypeFlags};
use smithay::reexports::input::Libinput;
use smithay::reexports::rustix::fs::OFlags;
use smithay::utils::DeviceFd;

use crate::State;

type Compositor = DrmCompositor<GbmAllocator<DrmDeviceFd>, GbmFramebufferExporter<DrmDeviceFd>, (), DrmDeviceFd>;

/// What was taken from the system to show a screen.
pub struct Backend {
    /// The session with the seat manager. Dropping it gives the seat back.
    pub session: LibSeatSession,
    renderer: GlesRenderer,
    compositor: Compositor,
    output: Output,
    /// A frame is on its way to the screen; the next one waits for it.
    frame_pending: bool,
    /// Something changed since the last frame was drawn.
    dirty: bool,
    /// Counters printed when `OCINYE_COMPOSITOR_TRACE` is set: frames queued,
    /// frames confirmed by the display, and frames with nothing to draw.
    trace: Option<[u64; 3]>,
    traced_at: std::time::Instant,
}

/// The background behind every window: Ocinye's deep navy.
const BACKGROUND: [f32; 4] = [0.027, 0.118, 0.200, 1.0];

type InitError = Box<dyn std::error::Error>;

/// Take the seat, the GPU and the input devices.
///
/// # Errors
///
/// Anything that stops the compositor from showing a screen: no seat, no GPU,
/// no connected display.
pub fn init(event_loop: &mut EventLoop<'static, State>, state: &mut State) -> Result<(), InitError> {
    let (mut session, notifier) = LibSeatSession::new()?;
    let seat = session.seat();

    let gpu = primary_gpu(&seat)?
        .or_else(|| all_gpus(&seat).ok().and_then(|g| g.into_iter().next()))
        .ok_or("no GPU on this seat")?;
    let fd = session.open(&gpu, OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK)?;
    let fd = DrmDeviceFd::new(DeviceFd::from(fd));
    let (mut drm, drm_events) = DrmDevice::new(fd.clone(), true)?;
    let gbm = GbmDevice::new(fd)?;

    // SAFETY: the GBM device outlives the display and context created from it;
    // all three live in `Backend` or are owned by the renderer.
    let egl = unsafe { EGLDisplay::new(gbm.clone())? };
    let context = EGLContext::new(&egl)?;
    // SAFETY: the context is current on this thread only, and this thread is
    // the only one that renders.
    let renderer = unsafe { GlesRenderer::new(context)? };
    state.shm_state.update_formats(renderer.shm_formats());

    // The first connected connector, at the mode the display prefers.
    let resources = drm.resource_handles()?;
    let connector = resources
        .connectors()
        .iter()
        .filter_map(|c| drm.get_connector(*c, true).ok())
        .find(|c| c.state() == connector::State::Connected && !c.modes().is_empty())
        .ok_or("no connected display")?;
    let mode = *connector
        .modes()
        .iter()
        .find(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED))
        .unwrap_or(&connector.modes()[0]);
    let crtc = connector
        .encoders()
        .iter()
        .filter_map(|e| drm.get_encoder(*e).ok())
        .flat_map(|e| resources.filter_crtcs(e.possible_crtcs()))
        .next()
        .ok_or("the display has no usable controller")?;
    let surface = drm.create_surface(crtc, mode, &[connector.handle()])?;

    let name = format!("{}-{}", connector.interface().as_str(), connector.interface_id());
    let (width_mm, height_mm) = connector.size().unwrap_or((0, 0));
    let output = Output::new(
        name,
        PhysicalProperties {
            size: (i32::try_from(width_mm).unwrap_or(0), i32::try_from(height_mm).unwrap_or(0)).into(),
            subpixel: Subpixel::Unknown,
            make: "Ocinye".into(),
            model: "Display".into(),
        },
    );
    let _global = output.create_global::<State>(&state.display_handle);
    let wl_mode = WlMode::from(mode);
    output.set_preferred(wl_mode);
    output.change_current_state(Some(wl_mode), None, None, Some((0, 0).into()));
    state.space.map_output(&output, (0, 0));

    let render_formats = renderer.egl_context().dmabuf_render_formats().clone();
    let compositor: Compositor = DrmCompositor::new(
        &output,
        surface,
        None,
        GbmAllocator::new(gbm.clone(), GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT),
        GbmFramebufferExporter::new(gbm.clone(), None),
        [Fourcc::Abgr8888, Fourcc::Argb8888, Fourcc::Xbgr8888, Fourcc::Xrgb8888],
        render_formats,
        drm.cursor_size(),
        Some(gbm),
    )?;

    // Input.
    let mut libinput = Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(session.clone().into());
    libinput.udev_assign_seat(&seat).map_err(|()| "libinput could not take the seat")?;
    let input = LibinputInputBackend::new(libinput.clone());
    event_loop.handle().insert_source(input, |event, (), state| {
        if !matches!(event, InputEvent::DeviceAdded { .. } | InputEvent::DeviceRemoved { .. }) {
            state.process_input_event(event);
        }
    })?;

    // The seat manager pauses the session when the screen is taken away
    // (a virtual terminal switch) and resumes it when it comes back.
    event_loop.handle().insert_source(notifier, move |event, &mut (), state| match event {
        SessionEvent::PauseSession => libinput.suspend(),
        SessionEvent::ActivateSession => {
            let _ = libinput.resume();
            if let Some(b) = state.backend.as_mut() {
                let _ = b.compositor.reset_state();
                b.frame_pending = false;
            }
            state.request_redraw();
        }
    })?;

    // A frame reached the screen: the next one may be drawn.
    event_loop.handle().insert_source(drm_events, |event, _, state| {
        if let DrmEvent::VBlank(_) = event {
            if let Some(b) = state.backend.as_mut() {
                let _ = b.compositor.frame_submitted();
                b.frame_pending = false;
                if let Some(t) = b.trace.as_mut() {
                    t[1] += 1;
                }
            }
            state.render();
        }
    })?;

    // Draw when something changed and no page flip is outstanding. The timer
    // also covers the first frame and any frame the display did not confirm.
    event_loop.handle().insert_source(Timer::from_duration(Duration::from_millis(1)), |_, (), state| {
        state.render();
        TimeoutAction::ToDuration(Duration::from_millis(16))
    })?;

    let trace = std::env::var_os("OCINYE_COMPOSITOR_TRACE").map(|_| [0; 3]);
    state.backend = Some(Backend {
        session,
        renderer,
        compositor,
        output,
        frame_pending: false,
        dirty: true,
        trace,
        traced_at: std::time::Instant::now(),
    });
    // The device itself stays open for as long as the compositor runs.
    std::mem::forget(drm);
    Ok(())
}

impl State {
    /// Something changed: draw a frame at the next opportunity.
    pub fn request_redraw(&mut self) {
        if let Some(b) = self.backend.as_mut() {
            b.dirty = true;
        }
    }

    /// Draw one frame, if one is due.
    pub fn render(&mut self) {
        let Some(backend) = self.backend.as_mut() else { return };
        if let Some(t) = backend.trace {
            if backend.traced_at.elapsed() >= Duration::from_secs(2) {
                backend.traced_at = std::time::Instant::now();
                eprintln!(
                    "trace: queued={} confirmed={} empty={} windows={} pending={} dirty={}",
                    t[0],
                    t[1],
                    t[2],
                    self.space.elements().count(),
                    backend.frame_pending,
                    backend.dirty
                );
            }
        }
        if backend.frame_pending || !backend.dirty {
            return;
        }
        let elements: Vec<SpaceRenderElements<GlesRenderer, WaylandSurfaceRenderElement<GlesRenderer>>> =
            match space_render_elements(&mut backend.renderer, [&self.space], &backend.output, 1.0) {
                Ok(e) => e,
                Err(_) => return,
            };
        backend.dirty = false;
        match backend.compositor.render_frame(&mut backend.renderer, &elements, BACKGROUND, FrameFlags::DEFAULT) {
            Ok(frame) => {
                if frame.is_empty {
                    if let Some(t) = backend.trace.as_mut() {
                        t[2] += 1;
                    }
                } else {
                    match backend.compositor.queue_frame(()) {
                        Ok(()) => {
                            backend.frame_pending = true;
                            if let Some(t) = backend.trace.as_mut() {
                                t[0] += 1;
                            }
                        }
                        Err(e) => eprintln!("ocinye-compositor: queue: {e}"),
                    }
                }
            }
            Err(e) => eprintln!("ocinye-compositor: frame: {e}"),
        }
        let output = backend.output.clone();
        let elapsed = self.start_time.elapsed();
        for window in self.space.elements() {
            window.send_frame(&output, elapsed, Some(Duration::ZERO), |_, _| Some(output.clone()));
        }
        self.space.refresh();
        self.popups.cleanup();
        let _ = self.display_handle.flush_clients();
    }
}
