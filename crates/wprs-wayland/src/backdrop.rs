use crate::x11::X11;
use x11rb::connection::{Connection as _, RequestConnection as _};
use x11rb::protocol::xproto::*;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::xcb_ffi::XCBConnection;

const PUT_IMAGE_HEADER: usize = 24;

impl X11 {
    pub(crate) fn backdrop(&self, desk: Window, at: (i32, i32), size: (u32, u32), stride: u32, data: &[u8]) {
        if let Err(e) = self.paint(desk, at, size, stride as usize, data) {
            tracing::warn!("cannot update the root window background: {e}");
        }
    }

    fn paint(
        &self,
        desk: Window,
        (x, y): (i32, i32),
        (width, height): (u32, u32),
        stride: usize,
        data: &[u8],
    ) -> anyhow::Result<()> {
        let Some(depth) = self.depth else { return Ok(()) };
        let (c, a, root) = (&self.conn, &self.atoms, self.root);
        let geometry = c.get_geometry(root)?.reply()?;
        let size = (geometry.width, geometry.height);
        let current = self.property(root, a._XROOTPMAP_ID)?.first().copied();
        let esetroot = self.property(root, a.ESETROOT_PMAP_ID)?.first().copied();
        let mut held = self.pixmap.lock().unwrap_or_else(|e| e.into_inner());
        let pixmap = match *held {
            Some((pixmap, at)) if at == size && current == Some(pixmap) => pixmap,
            old => {
                let pixmap = retained(depth, root, size)?;
                c.poly_fill_rectangle(
                    pixmap,
                    self.gc,
                    &[Rectangle { x: 0, y: 0, width: size.0, height: size.1 }],
                )?;
                if let Some((old, _)) = old.filter(|(old, _)| current == Some(*old)) {
                    c.copy_area(old, pixmap, self.gc, 0, 0, 0, 0, size.0, size.1)?;
                }
                *held = Some((pixmap, size));
                pixmap
            }
        };
        let row = width as usize * 4;
        let lines = ((c.maximum_request_bytes() - PUT_IMAGE_HEADER) / row.max(1)).max(1);
        for (i, chunk) in data.chunks(stride * lines).enumerate() {
            let packed: Vec<u8> = chunk.chunks(stride).flat_map(|r| &r[..row]).copied().collect();
            let (top, height) = (y + (i * lines) as i32, (packed.len() / row) as u16);
            c.put_image(
                ImageFormat::Z_PIXMAP,
                pixmap,
                self.gc,
                width as u16,
                height,
                x as i16,
                top as i16,
                0,
                depth,
                &packed,
            )?;
        }
        let tile = c.generate_id()?;
        c.create_pixmap(depth, tile, root, width as u16, height as u16)?;
        c.copy_area(pixmap, tile, self.gc, x as i16, y as i16, 0, 0, width as u16, height as u16)?;
        c.change_window_attributes(desk, &ChangeWindowAttributesAux::new().background_pixmap(tile))?;
        c.free_pixmap(tile)?;
        c.change_window_attributes(root, &ChangeWindowAttributesAux::new().background_pixmap(pixmap))?;
        c.clear_area(false, root, 0, 0, 0, 0)?;
        for atom in [a._XROOTPMAP_ID, a.ESETROOT_PMAP_ID] {
            c.change_property32(PropMode::REPLACE, root, atom, AtomEnum::PIXMAP, &[pixmap])?;
        }
        if let Some(old) = esetroot.filter(|old| current == Some(*old) && *old != pixmap) {
            c.kill_client(old)?;
        }
        Ok(c.flush()?)
    }
}

fn retained(depth: u8, root: Window, (width, height): (u16, u16)) -> anyhow::Result<Pixmap> {
    let (conn, _) = XCBConnection::connect(None)?;
    conn.set_close_down_mode(CloseDown::RETAIN_PERMANENT)?;
    let pixmap = conn.generate_id()?;
    conn.create_pixmap(depth, pixmap, root, width, height)?;
    conn.get_input_focus()?.reply()?;
    Ok(pixmap)
}
