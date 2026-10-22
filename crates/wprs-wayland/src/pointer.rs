use crate::Event;
use crate::state::State;
use smithay_client_toolkit::reexports::client::protocol::{wl_pointer::WlPointer, wl_seat::WlSeat};
use smithay_client_toolkit::reexports::client::{Connection, QueueHandle};
use smithay_client_toolkit::seat::pointer::{
    CursorIcon, PointerEvent, PointerEventKind, PointerHandler, ThemeSpec,
};
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shm::{Shm, ShmHandler};

type Queue = QueueHandle<State>;

fn bit(button: u32) -> u8 {
    button.checked_sub(0x110).filter(|b| *b < 8).map_or(0, |b| 1 << b)
}

impl SeatHandler for State {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat
    }

    fn new_seat(&mut self, _: &Connection, _: &Queue, _: WlSeat) {}

    fn new_capability(&mut self, _: &Connection, qh: &Queue, seat: WlSeat, capability: Capability) {
        if capability == Capability::Pointer && self.pointer.is_none() {
            let (shm, cursor) = (self.shm.wl_shm(), self.compositor.create_surface(qh));
            let theme = ThemeSpec::default();
            self.pointer = self.seat.get_pointer_with_theme::<_, ()>(qh, &seat, shm, cursor, theme).ok();
        }
    }

    fn remove_capability(&mut self, _: &Connection, _: &Queue, _: WlSeat, capability: Capability) {
        if capability == Capability::Pointer {
            self.pointer = None;
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &Queue, _: WlSeat) {
        (self.pointer, self.buttons) = (None, 0);
    }
}

impl ShmHandler for State {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl PointerHandler for State {
    fn pointer_frame(&mut self, conn: &Connection, _: &Queue, _: &WlPointer, events: &[PointerEvent]) {
        for event in events {
            let Some(screen) = self.screens.iter().find(|s| *s.parts.layer.wl_surface() == event.surface)
            else {
                continue;
            };
            let (width, height) = screen.logical.unwrap_or((1, 1));
            let (x, y) = (event.position.0 / f64::from(width), event.position.1 / f64::from(height));
            let at = Some([x as f32, y as f32]);
            let pos = match event.kind {
                PointerEventKind::Axis { .. } => continue,
                PointerEventKind::Leave { .. } => {
                    self.buttons = 0;
                    None
                }
                PointerEventKind::Press { button, .. } => {
                    self.buttons |= bit(button);
                    at
                }
                PointerEventKind::Release { button, .. } => {
                    self.buttons &= !bit(button);
                    at
                }
                PointerEventKind::Enter { .. } => {
                    let cursor = self.pointer.as_ref().map(|p| p.set_cursor(conn, CursorIcon::Default));
                    if let Some(Err(e)) = cursor {
                        tracing::debug!("cannot set the cursor over the background: {e}");
                    }
                    at
                }
                PointerEventKind::Motion { .. } => at,
            };
            (self.sink)(Event::Pointer(screen.name.clone(), pos, self.buttons));
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn mouse_buttons_map_to_bits() {
        assert_eq!([0x110, 0x111, 0x112, 0x117, 0x118, 0x10f, 0].map(super::bit), [1, 2, 4, 128, 0, 0, 0]);
    }
}
