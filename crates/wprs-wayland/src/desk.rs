use crate::Sink;
use crate::monitors::Monitor;
use crate::scale::UNIT;
use crate::window::{Display, Native};
use crate::x11::{Rect, X11};
use raw_window_handle::{HandleError, WindowHandle, XcbWindowHandle};
use std::num::NonZeroU32;
use std::sync::Arc;
use x11rb::connection::Connection as _;
use x11rb::errors::ReplyOrIdError;
use x11rb::properties::{WmSizeHints, WmSizeHintsSpecification::UserSpecified};
use x11rb::protocol::shape::{self, SK, SO};
use x11rb::protocol::xproto::*;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{COPY_DEPTH_FROM_PARENT, COPY_FROM_PARENT};

fn mask(on: bool) -> EventMask {
    let base = EventMask::EXPOSURE | EventMask::VISIBILITY_CHANGE;
    let moves = EventMask::POINTER_MOTION | EventMask::ENTER_WINDOW | EventMask::LEAVE_WINDOW;
    if on { base | moves | EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE } else { base }
}

pub(crate) enum Felt {
    At(Window, [i16; 2]),
    Button(Window, [i16; 2], u8, bool),
    Left(Window),
}

pub(crate) fn felt(event: &x11rb::protocol::Event) -> Option<Felt> {
    use x11rb::protocol::Event as E;
    let real = |kind: u8| kind & 0x80 == 0;
    let bit = |button: u8| [0, 1, 4, 2, 0, 0, 0, 0, 8, 16].get(usize::from(button)).copied().unwrap_or(0);
    match event {
        E::MotionNotify(e) if real(e.response_type) => Some(Felt::At(e.event, [e.event_x, e.event_y])),
        E::EnterNotify(e) if real(e.response_type) => Some(Felt::At(e.event, [e.event_x, e.event_y])),
        E::LeaveNotify(e) if real(e.response_type) => Some(Felt::Left(e.event)),
        E::ButtonPress(e) if real(e.response_type) && bit(e.detail) != 0 => {
            Some(Felt::Button(e.event, [e.event_x, e.event_y], bit(e.detail), true))
        }
        E::ButtonRelease(e) if real(e.response_type) && bit(e.detail) != 0 => {
            Some(Felt::Button(e.event, [e.event_x, e.event_y], bit(e.detail), false))
        }
        _ => None,
    }
}

#[derive(Debug)]
pub struct Desk {
    pub(crate) x: Arc<X11>,
    pub(crate) window: Window,
}

impl Desk {
    pub(crate) fn new(x: &Arc<X11>, area: Rect) -> Result<Self, ReplyOrIdError> {
        let (c, a, window) = (&x.conn, &x.atoms, x.conn.generate_id()?);
        let managed = x.managed();
        let background = if managed { BackPixmap::NONE } else { BackPixmap::PARENT_RELATIVE };
        let aux = CreateWindowAux::new().override_redirect(u32::from(!managed)).background_pixmap(background);
        let aux = aux.event_mask(mask(false));
        let (depth, visual, class) = (COPY_DEPTH_FROM_PARENT, COPY_FROM_PARENT, WindowClass::INPUT_OUTPUT);
        c.create_window(depth, window, x.root, 0, 0, 1, 1, 0, class, visual, &aux)?;
        let desk = Self { x: x.clone(), window };
        let set = |atom, kind: AtomEnum, list: &[u32]| {
            c.change_property32(PropMode::REPLACE, window, atom, kind, list)
        };
        set(a._NET_WM_WINDOW_TYPE, AtomEnum::ATOM, &[a._NET_WM_WINDOW_TYPE_DESKTOP])?;
        let states = [
            a._NET_WM_STATE_BELOW,
            a._NET_WM_STATE_STICKY,
            a._NET_WM_STATE_SKIP_TASKBAR,
            a._NET_WM_STATE_SKIP_PAGER,
        ];
        set(a._NET_WM_STATE, AtomEnum::ATOM, &states)?;
        set(a._NET_WM_DESKTOP, AtomEnum::CARDINAL, &[u32::MAX])?;
        let names = b"wallpaper\0wprsd\0";
        c.change_property8(PropMode::REPLACE, window, AtomEnum::WM_CLASS, AtomEnum::STRING, names)?;
        if let Err(e) = shape::rectangles(c, SO::SET, SK::INPUT, ClipOrdering::UNSORTED, window, 0, 0, &[]) {
            tracing::debug!("the wallpaper window may take pointer input: {e}");
        }
        desk.place(area)?;
        c.map_window(window)?;
        desk.lower()?;
        Ok(desk)
    }

    pub(crate) fn place(&self, Rect { x, y, width, height }: Rect) -> Result<(), ReplyOrIdError> {
        let c = &self.x.conn;
        c.configure_window(self.window, &ConfigureWindowAux::new().x(x).y(y).width(width).height(height))?;
        let size = Some((UserSpecified, width as i32, height as i32));
        WmSizeHints { position: Some((UserSpecified, x, y)), size, ..Default::default() }
            .set_normal_hints(c, self.window)?;
        Ok(c.flush()?)
    }

    pub(crate) fn track(&self, on: bool) {
        let (c, window) = (&self.x.conn, self.window);
        let events = ChangeWindowAttributesAux::new().event_mask(mask(on));
        let shaped = match on {
            true => shape::mask(c, SO::SET, SK::INPUT, window, 0, 0, x11rb::NONE).map(drop),
            false => {
                shape::rectangles(c, SO::SET, SK::INPUT, ClipOrdering::UNSORTED, window, 0, 0, &[]).map(drop)
            }
        };
        let done = c.change_window_attributes(window, &events).map(drop).and(shaped).and_then(|()| c.flush());
        match done {
            Ok(()) => tracing::debug!(on, "pointer input of the background"),
            Err(e) => tracing::debug!(on, "cannot change the pointer input of the wallpaper window: {e}"),
        }
    }

    pub(crate) fn lower(&self) -> Result<(), ReplyOrIdError> {
        let below = ConfigureWindowAux::new().stack_mode(StackMode::BELOW);
        self.x.conn.configure_window(self.window, &below)?;
        Ok(self.x.conn.flush()?)
    }

    pub(crate) fn window(self: &Arc<Self>, monitor: &Monitor, sink: Sink) -> crate::Window {
        let Monitor { name, area, mm, refresh } = monitor.clone();
        crate::Window {
            native: Native::X11(self.clone()),
            server: Display::X11(self.x.clone()),
            sink,
            name,
            logical: (area.width, area.height),
            scale: UNIT,
            refresh,
            position: (area.x, area.y),
            mm,
            identity: String::new(),
        }
    }

    pub(crate) fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let window = NonZeroU32::new(self.window).ok_or(HandleError::Unavailable)?;
        Ok(unsafe { WindowHandle::borrow_raw(XcbWindowHandle::new(window).into()) })
    }
}

impl Drop for Desk {
    fn drop(&mut self) {
        let _ = self.x.conn.destroy_window(self.window);
        let _ = self.x.conn.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::{Felt, felt};
    use x11rb::protocol::Event;
    use x11rb::protocol::xproto::{BUTTON_PRESS_EVENT, ButtonPressEvent, MotionNotifyEvent};

    #[test]
    fn only_real_presses_of_known_buttons_reach_the_wallpaper() {
        let press = |detail, sent: bool| ButtonPressEvent {
            response_type: BUTTON_PRESS_EVENT | if sent { 0x80 } else { 0 },
            detail,
            event: 7,
            event_x: 10,
            event_y: 20,
            ..ButtonPressEvent::default()
        };
        let bits = [1, 2, 3, 4, 8, 9].map(|b| match felt(&Event::ButtonPress(press(b, false))) {
            Some(Felt::Button(7, [10, 20], bit, true)) => bit,
            _ => 0,
        });
        assert_eq!(bits, [1, 4, 2, 0, 8, 16]);
        assert!(felt(&Event::ButtonPress(press(1, true))).is_none(), "a synthesized click was taken");
        assert!(felt(&Event::ButtonRelease(press(1, true))).is_none());
        let moved = MotionNotifyEvent { event: 7, event_x: 3, event_y: 4, ..MotionNotifyEvent::default() };
        assert!(matches!(felt(&Event::MotionNotify(moved)), Some(Felt::At(7, [3, 4]))));
    }
}
