//! Input: keyboard to the focused surface, pointer to the surface under it.

use smithay::backend::input::{
    AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, InputBackend, InputEvent,
    KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent, PointerMotionEvent,
};
use smithay::input::keyboard::FilterResult;
use smithay::input::pointer::{AxisFrame, ButtonEvent, MotionEvent};
use smithay::utils::{Logical, Point, SERIAL_COUNTER};

use crate::State;

impl State {
    fn output_size(&self) -> Option<smithay::utils::Size<i32, Logical>> {
        self.space
            .outputs()
            .next()
            .and_then(|o| self.space.output_geometry(o))
            .map(|g| g.size)
    }

    fn move_pointer(&mut self, location: Point<f64, Logical>, time: u32) {
        self.pointer_location = location;
        let under = self.surface_under(location);
        if let Some(pointer) = self.seat.get_pointer() {
            pointer.motion(
                self,
                under,
                &MotionEvent {
                    location,
                    serial: SERIAL_COUNTER.next_serial(),
                    time,
                },
            );
            pointer.frame(self);
        }
        self.request_redraw();
    }

    /// Route one input event.
    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        self.last_input = std::time::Instant::now();
        match event {
            InputEvent::Keyboard { event, .. } => {
                if let Some(keyboard) = self.seat.get_keyboard() {
                    keyboard.input::<(), _>(
                        self,
                        event.key_code(),
                        event.state(),
                        SERIAL_COUNTER.next_serial(),
                        Event::time_msec(&event),
                        |_, _, _| FilterResult::Forward,
                    );
                }
            }
            InputEvent::PointerMotion { event, .. } => {
                let Some(size) = self.output_size() else {
                    return;
                };
                let mut pos = self.pointer_location + event.delta();
                pos.x = pos.x.clamp(0.0, f64::from(size.w - 1));
                pos.y = pos.y.clamp(0.0, f64::from(size.h - 1));
                self.move_pointer(pos, event.time_msec());
            }
            InputEvent::PointerMotionAbsolute { event, .. } => {
                let Some(size) = self.output_size() else {
                    return;
                };
                self.move_pointer(event.position_transformed(size), event.time_msec());
            }
            InputEvent::PointerButton { event, .. } => {
                let Some(pointer) = self.seat.get_pointer() else {
                    return;
                };
                let serial = SERIAL_COUNTER.next_serial();
                if event.state() == ButtonState::Pressed && !pointer.is_grabbed() {
                    if let Some((window, _)) = self
                        .space
                        .element_under(self.pointer_location)
                        .map(|(w, l)| (w.clone(), l))
                    {
                        if let (Some(keyboard), Some(toplevel)) =
                            (self.seat.get_keyboard(), window.toplevel())
                        {
                            keyboard.set_focus(self, Some(toplevel.wl_surface().clone()), serial);
                        }
                    }
                }
                pointer.button(
                    self,
                    &ButtonEvent {
                        button: event.button_code(),
                        state: event.state(),
                        serial,
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);
            }
            InputEvent::PointerAxis { event, .. } => {
                let amount = |axis| {
                    event
                        .amount(axis)
                        .unwrap_or_else(|| event.amount_v120(axis).unwrap_or(0.0) * 15.0 / 120.0)
                };
                let mut frame = AxisFrame::new(event.time_msec()).source(event.source());
                for axis in [Axis::Horizontal, Axis::Vertical] {
                    let value = amount(axis);
                    if value != 0.0 {
                        frame = frame.value(axis, value);
                        if let Some(discrete) = event.amount_v120(axis) {
                            // v120 steps are small integers by definition.
                            #[allow(clippy::cast_possible_truncation)]
                            {
                                frame = frame.v120(axis, discrete as i32);
                            }
                        }
                    } else if event.source() == AxisSource::Finger
                        && event.amount(axis) == Some(0.0)
                    {
                        frame = frame.stop(axis);
                    }
                }
                if let Some(pointer) = self.seat.get_pointer() {
                    pointer.axis(self, frame);
                    pointer.frame(self);
                }
            }
            _ => {}
        }
    }
}
