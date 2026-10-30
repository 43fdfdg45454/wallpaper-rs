use crate::Event;
use crate::state::State;
use smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface;
use smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch;
use smithay_client_toolkit::reexports::client::protocol::{wl_pointer::WlPointer, wl_seat::WlSeat};
use smithay_client_toolkit::reexports::client::{Connection, QueueHandle};
use smithay_client_toolkit::seat::pointer::{
    CursorIcon, PointerEvent, PointerEventKind, PointerHandler, ThemeSpec,
};
use smithay_client_toolkit::seat::touch::TouchHandler;
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
        if capability == Capability::Touch && self.touch.is_none() {
            self.touch = self.seat.get_touch(qh, &seat).ok();
        }
    }

    fn remove_capability(&mut self, _: &Connection, _: &Queue, _: WlSeat, capability: Capability) {
        match capability {
            Capability::Pointer => self.pointer = None,
            Capability::Touch => (self.touch, self.touching) = (None, None),
            _ => {}
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &Queue, _: WlSeat) {
        (self.pointer, self.buttons, self.touch, self.touching) = (None, 0, None, None);
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

impl State {
    fn placed(&self, surface: &WlSurface, (x, y): (f64, f64)) -> Option<(String, [f32; 2])> {
        let screen = self.screens.iter().find(|s| s.parts.layer.wl_surface() == surface)?;
        let (width, height) = screen.logical.unwrap_or((1, 1));
        Some((screen.name.clone(), [(x / f64::from(width)) as f32, (y / f64::from(height)) as f32]))
    }

    fn lift(&mut self) {
        if let Some((_, name, at)) = self.touching.take() {
            (self.sink)(Event::Pointer(name.clone(), Some(at), 0));
            (self.sink)(Event::Pointer(name, None, 0));
        }
    }
}

impl TouchHandler for State {
    fn down(
        &mut self,
        _: &Connection,
        _: &Queue,
        _: &WlTouch,
        _: u32,
        _: u32,
        s: WlSurface,
        id: i32,
        at: (f64, f64),
    ) {
        let Some((name, at)) = self.placed(&s, at).filter(|_| self.touching.is_none()) else { return };
        self.touching = Some((id, name.clone(), at));
        (self.sink)(Event::Pointer(name.clone(), Some(at), 0));
        (self.sink)(Event::Pointer(name, Some(at), 1));
    }

    fn up(&mut self, _: &Connection, _: &Queue, _: &WlTouch, _: u32, _: u32, id: i32) {
        if self.touching.as_ref().is_some_and(|t| t.0 == id) {
            self.lift();
        }
    }

    fn motion(&mut self, _: &Connection, _: &Queue, _: &WlTouch, _: u32, id: i32, at: (f64, f64)) {
        let Some((_, name, _)) = self.touching.clone().filter(|t| t.0 == id) else { return };
        let surface =
            self.screens.iter().find(|s| s.name == name).map(|s| s.parts.layer.wl_surface().clone());
        if let Some((_, at)) = surface.and_then(|s| self.placed(&s, at)) {
            self.touching = Some((id, name.clone(), at));
            (self.sink)(Event::Pointer(name, Some(at), 1));
        }
    }

    fn shape(&mut self, _: &Connection, _: &Queue, _: &WlTouch, _: i32, _: f64, _: f64) {}

    fn orientation(&mut self, _: &Connection, _: &Queue, _: &WlTouch, _: i32, _: f64) {}

    fn cancel(&mut self, _: &Connection, _: &Queue, _: &WlTouch) {
        self.lift();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn mouse_buttons_map_to_bits() {
        assert_eq!([0x110, 0x111, 0x112, 0x117, 0x118, 0x10f, 0].map(super::bit), [1, 2, 4, 128, 0, 0, 0]);
    }
}
