use crate::Sink;
use crate::scale::UNIT;
use crate::window::{Display, Native};
use crate::x11::{Rect, X11};
use raw_window_handle::{HandleError, WindowHandle, XcbWindowHandle};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;
use x11rb::connection::Connection as _;
use x11rb::errors::ReplyOrIdError;
use x11rb::properties::{WmSizeHints, WmSizeHintsSpecification::UserSpecified};
use x11rb::protocol::shape::{self, SK, SO};
use x11rb::protocol::xproto::*;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{COPY_DEPTH_FROM_PARENT, COPY_FROM_PARENT};

#[derive(Debug)]
pub struct Desk {
    pub(crate) x: Arc<X11>,
    pub(crate) window: Window,
}

impl Desk {
    pub(crate) fn new(x: &Arc<X11>, area: Rect) -> Result<Self, ReplyOrIdError> {
        let (c, a, window) = (&x.conn, &x.atoms, x.conn.generate_id()?);
        let background = if x.managed { BackPixmap::NONE } else { BackPixmap::PARENT_RELATIVE };
        let aux =
            CreateWindowAux::new().override_redirect(u32::from(!x.managed)).background_pixmap(background);
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

    pub(crate) fn lower(&self) -> Result<(), ReplyOrIdError> {
        let below = ConfigureWindowAux::new().stack_mode(StackMode::BELOW);
        self.x.conn.configure_window(self.window, &below)?;
        Ok(self.x.conn.flush()?)
    }

    pub(crate) fn window(
        self: &Arc<Self>,
        name: &str,
        area: Rect,
        mm: (u32, u32),
        sink: Sink,
    ) -> crate::Window {
        crate::Window {
            native: Native::X11(self.clone()),
            server: Display::X11(self.x.clone()),
            sink,
            name: name.into(),
            logical: (area.width, area.height),
            scale: UNIT,
            refresh: Duration::from_micros(16_667),
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
