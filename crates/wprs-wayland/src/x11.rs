use anyhow::Context;
use raw_window_handle::{DisplayHandle, HandleError, XcbDisplayHandle};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use x11rb::connection::{Connection as _, RequestConnection as _};
use x11rb::errors::ReplyError;
use x11rb::protocol::randr::{self, NotifyMask};
use x11rb::protocol::xproto::*;
use x11rb::xcb_ffi::XCBConnection;

x11rb::atom_manager! {
    pub(crate) Atoms: AtomsCookie {
        _NET_SUPPORTED, _NET_SUPPORTING_WM_CHECK, _NET_CLIENT_LIST_STACKING, _NET_CURRENT_DESKTOP,
        _NET_WM_DESKTOP, _NET_WM_WINDOW_TYPE, _NET_WM_WINDOW_TYPE_DESKTOP, _NET_WM_STATE,
        _NET_WM_STATE_BELOW, _NET_WM_STATE_STICKY, _NET_WM_STATE_SKIP_TASKBAR, _NET_WM_STATE_SKIP_PAGER,
        _NET_WM_STATE_FULLSCREEN, _NET_WM_STATE_MAXIMIZED_VERT, _NET_WM_STATE_MAXIMIZED_HORZ,
        _NET_WM_STATE_HIDDEN, _XROOTPMAP_ID, ESETROOT_PMAP_ID,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Rect {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl Rect {
    pub(crate) fn holds(&self, (x, y): (i32, i32)) -> bool {
        (self.x..self.x + self.width as i32).contains(&x)
            && (self.y..self.y + self.height as i32).contains(&y)
    }

    pub(crate) fn center(&self) -> (i32, i32) {
        (self.x + self.width as i32 / 2, self.y + self.height as i32 / 2)
    }
}

#[derive(Debug)]
pub struct X11 {
    pub(crate) conn: XCBConnection,
    pub(crate) screen: usize,
    pub(crate) root: Window,
    pub(crate) atoms: Atoms,
    pub(crate) managed: AtomicBool,
    pub(crate) gc: Gcontext,
    pub(crate) depth: Option<u8>,
    pub(crate) pixmap: Mutex<Option<(Pixmap, (u16, u16))>>,
}

impl X11 {
    pub(crate) fn connect() -> anyhow::Result<Arc<Self>> {
        let (conn, screen) = XCBConnection::connect(None)?;
        anyhow::ensure!(conn.extension_information("XWAYLAND")?.is_none(), "the X server runs on Wayland");
        let setup = conn.setup();
        let info = setup.roots.get(screen).context("the X server has no such screen")?;
        let (root, depth, black) = (info.root, info.root_depth, info.black_pixel);
        let packed = setup.image_byte_order == ImageOrder::LSB_FIRST
            && setup.pixmap_formats.iter().any(|f| f.depth == depth && f.bits_per_pixel == 32);
        let version = randr::query_version(&conn, 1, 5)?.reply().context("the X server lacks RandR")?;
        anyhow::ensure!(version.minor_version >= 5, "the X server cannot list monitors");
        let atoms = Atoms::new(&conn)?.reply()?;
        let gc = conn.generate_id()?;
        conn.create_gc(gc, root, &CreateGCAux::new().foreground(black).graphics_exposures(0))?;
        let mask = EventMask::STRUCTURE_NOTIFY | EventMask::SUBSTRUCTURE_NOTIFY | EventMask::PROPERTY_CHANGE;
        conn.change_window_attributes(root, &ChangeWindowAttributesAux::new().event_mask(mask))?;
        let changes = NotifyMask::SCREEN_CHANGE | NotifyMask::CRTC_CHANGE | NotifyMask::OUTPUT_CHANGE;
        randr::select_input(&conn, root, changes)?;
        let (depth, pixmap) = (packed.then_some(depth), Mutex::new(None));
        let x = Self { conn, screen, root, atoms, managed: AtomicBool::new(false), gc, depth, pixmap };
        x.managed.store(x.manager()?.0, Relaxed);
        tracing::debug!(managed = x.managed(), "connected to the X server");
        Ok(Arc::new(x))
    }

    pub(crate) fn managed(&self) -> bool {
        self.managed.load(Relaxed)
    }

    pub(crate) fn manager(&self) -> Result<(bool, Option<Window>), ReplyError> {
        let (check, a) = (self.atoms._NET_SUPPORTING_WM_CHECK, &self.atoms);
        let wm = self.property(self.root, check)?.first().copied();
        let alive = wm.filter(|&w| self.property(w, check).is_ok_and(|v| v.first() == Some(&w)));
        let desktops = self.property(self.root, a._NET_SUPPORTED)?.contains(&a._NET_WM_WINDOW_TYPE_DESKTOP);
        Ok((alive.is_some() && desktops, alive))
    }

    pub(crate) fn property(&self, window: Window, atom: Atom) -> Result<Vec<u32>, ReplyError> {
        let reply = self.conn.get_property(false, window, atom, AtomEnum::ANY, 0, 4096)?.reply()?;
        Ok(reply.value32().map(Iterator::collect).unwrap_or_default())
    }

    pub(crate) fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        let conn = NonNull::new(self.conn.get_raw_xcb_connection()).ok_or(HandleError::Unavailable)?;
        let handle = XcbDisplayHandle::new(Some(conn), self.screen as i32);
        Ok(unsafe { DisplayHandle::borrow_raw(handle.into()) })
    }
}
