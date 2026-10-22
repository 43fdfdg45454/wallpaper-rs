use crate::scale::{UNIT, buffer_size};
use crate::state::State;
use crate::{Cover, Event, Sink};
use fractional::wp_fractional_scale_v1::WpFractionalScaleV1;
use raw_window_handle::{DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle};
use raw_window_handle::{WaylandDisplayHandle, WaylandWindowHandle};
use smithay_client_toolkit::compositor::FrameCallbackData;
use smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface;
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
}

impl Drop for Parts {
    fn drop(&mut self) {
        self.viewport.iter().for_each(WpViewport::destroy);
        self.fractional.iter().for_each(WpFractionalScaleV1::destroy);
    }
}

#[derive(Debug, Clone)]
pub struct Display(pub(crate) Connection);

#[derive(Clone)]
pub struct Window {
    pub(crate) parts: Arc<Parts>,
    pub(crate) qh: QueueHandle<State>,
    pub(crate) conn: Connection,
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
        Arc::ptr_eq(&self.parts, &other.parts)
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
        let (width, height) = self.logical;
        match &self.parts.viewport {
            Some(viewport) => viewport.set_destination(width as i32, height as i32),
            None => self.surface().set_buffer_scale((self.scale / UNIT) as i32),
        }
    }

    pub fn frame(&self) {
        let surface = self.surface();
        surface.frame(&self.qh, FrameCallbackData(surface.clone()));
    }

    pub fn commit(&self) {
        self.surface().commit();
        let _ = self.conn.flush();
    }

    pub fn report(&self, cover: Cover, covered: bool) {
        (self.sink)(Event::Covered(self.name.clone(), cover, covered))
    }

    fn surface(&self) -> &WlSurface {
        self.parts.layer.wl_surface()
    }
}

fn display(conn: &Connection) -> Result<DisplayHandle<'_>, HandleError> {
    let display = NonNull::new(conn.backend().display_ptr().cast()).ok_or(HandleError::Unavailable)?;
    Ok(unsafe { DisplayHandle::borrow_raw(WaylandDisplayHandle::new(display).into()) })
}

impl HasDisplayHandle for Display {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        display(&self.0)
    }
}

impl HasDisplayHandle for Window {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        display(&self.conn)
    }
}

impl HasWindowHandle for Window {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let surface = NonNull::new(self.surface().id().as_ptr().cast()).ok_or(HandleError::Unavailable)?;
        Ok(unsafe { WindowHandle::borrow_raw(WaylandWindowHandle::new(surface).into()) })
    }
}
