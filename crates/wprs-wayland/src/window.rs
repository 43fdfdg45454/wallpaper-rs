use crate::desk::Desk;
use crate::scale::{UNIT, buffer_size};
use crate::state::State;
use crate::x11::X11;
use crate::{Cover, Event, Sink};
use fractional::wp_fractional_scale_v1::WpFractionalScaleV1;
use raw_window_handle::{DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle};
use raw_window_handle::{WaylandDisplayHandle, WaylandWindowHandle};
use smithay_client_toolkit::compositor::{FrameCallbackData, Region};
use smithay_client_toolkit::reexports::client::{Connection, Proxy, QueueHandle};
use smithay_client_toolkit::reexports::protocols::wp::{
    fractional_scale::v1::client as fractional, viewporter,
};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shell::wlr_layer::LayerSurface;
use std::ptr::NonNull;
use std::sync::Arc;
use std::time::Duration;
use viewporter::client::wp_viewport::WpViewport;

pub(crate) struct Parts {
    pub(crate) layer: LayerSurface,
    pub(crate) viewport: Option<WpViewport>,
    pub(crate) fractional: Option<WpFractionalScaleV1>,
    pub(crate) empty: Option<Region>,
}

impl Drop for Parts {
    fn drop(&mut self) {
        self.viewport.iter().for_each(WpViewport::destroy);
        self.fractional.iter().for_each(WpFractionalScaleV1::destroy);
    }
}

#[derive(Debug, Clone)]
pub enum Display {
    Wayland(Connection),
    X11(Arc<X11>),
}

#[derive(Clone)]
pub(crate) enum Native {
    Layer(Arc<Parts>, QueueHandle<State>),
    X11(Arc<Desk>),
}

#[derive(Clone)]
pub struct Window {
    pub(crate) native: Native,
    pub(crate) server: Display,
    pub(crate) sink: Sink,
    pub(crate) name: String,
    pub(crate) logical: (u32, u32),
    pub(crate) scale: u32,
    pub(crate) refresh: Duration,
    pub(crate) position: (i32, i32),
    pub(crate) mm: (u32, u32),
    pub(crate) identity: String,
}

impl Window {
    pub fn size(&self) -> (u32, u32) {
        buffer_size(self.logical, self.scale)
    }

    pub fn same_surface(&self, other: &Window) -> bool {
        match (&self.native, &other.native) {
            (Native::Layer(a, ..), Native::Layer(b, ..)) => Arc::ptr_eq(a, b),
            (Native::X11(a), Native::X11(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }

    pub fn geometry(&self) -> ((u32, u32), u32) {
        (self.logical, self.scale)
    }

    pub fn placement(&self) -> ((i32, i32), (u32, u32), &str) {
        (self.position, self.mm, &self.identity)
    }

    pub fn refresh(&self) -> Duration {
        self.refresh
    }

    pub fn apply(&self) {
        let (Native::Layer(parts, ..), (width, height)) = (&self.native, self.logical) else { return };
        match &parts.viewport {
            Some(viewport) => viewport.set_destination(width as i32, height as i32),
            None => parts.layer.wl_surface().set_buffer_scale((self.scale / UNIT) as i32),
        }
    }

    pub fn frame(&self) {
        match &self.native {
            Native::Layer(parts, qh) => {
                let surface = parts.layer.wl_surface();
                surface.frame(qh, FrameCallbackData(surface.clone()));
            }
            Native::X11(_) => (self.sink)(Event::Frame(self.name.clone())),
        }
    }

    pub fn commit(&self) {
        if let (Native::Layer(parts, _), Display::Wayland(conn)) = (&self.native, &self.server) {
            parts.layer.wl_surface().commit();
            let _ = conn.flush();
        }
    }

    pub fn track_pointer(&self, on: bool) {
        let Native::Layer(parts, _) = &self.native else { return };
        let region = parts.empty.as_ref().filter(|_| !on).map(Region::wl_region);
        parts.layer.wl_surface().set_input_region(region);
        self.commit();
    }

    pub fn report(&self, cover: Cover, covered: bool) {
        (self.sink)(Event::Covered(self.name.clone(), cover, covered))
    }

    pub fn backdrop(&self) -> Option<impl FnOnce(u32, u32, u32, &[u8]) + '_> {
        let Native::X11(desk) = &self.native else { return None };
        Some(move |width, height, stride, data: &[u8]| {
            desk.x.backdrop(desk.window, self.position, (width, height), stride, data)
        })
    }
}

impl HasDisplayHandle for Display {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        let conn = match self {
            Display::Wayland(conn) => conn,
            Display::X11(x) => return x.display_handle(),
        };
        let display = NonNull::new(conn.backend().display_ptr().cast()).ok_or(HandleError::Unavailable)?;
        Ok(unsafe { DisplayHandle::borrow_raw(WaylandDisplayHandle::new(display).into()) })
    }
}

impl HasDisplayHandle for Window {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        self.server.display_handle()
    }
}

impl HasWindowHandle for Window {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let parts = match &self.native {
            Native::Layer(parts, ..) => parts,
            Native::X11(desk) => return desk.window_handle(),
        };
        let surface = parts.layer.wl_surface().id().as_ptr().cast();
        let surface = NonNull::new(surface).ok_or(HandleError::Unavailable)?;
        Ok(unsafe { WindowHandle::borrow_raw(WaylandWindowHandle::new(surface).into()) })
    }
}
