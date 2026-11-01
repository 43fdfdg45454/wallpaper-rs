use crate::{Frame, Pixels};
use gstreamer as gst;
use gstreamer_video as gst_video;
use gstreamer_video::{VideoColorRange, VideoTransferFunction as Tf};
use std::time::Duration;

pub const DEEP: &str = "I420_10LE";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transfer {
    Pq,
    Hlg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hdr {
    pub transfer: Transfer,
    pub planes: [(usize, u32); 3],
    pub full: bool,
}

pub(crate) fn transfer(caps: &gst::CapsRef) -> Option<Transfer> {
    let text = caps.structure(0)?.get::<&str>("colorimetry").ok()?;
    match text.parse::<gst_video::VideoColorimetry>().ok()?.transfer() {
        Tf::Smpte2084 => Some(Transfer::Pq),
        Tf::AribStdB67 => Some(Transfer::Hlg),
        _ => None,
    }
}

pub(crate) fn frame(info: &gst_video::VideoInfo, buffer: gst::Buffer) -> Option<Frame> {
    let transfer = match info.colorimetry().transfer() {
        Tf::Smpte2084 => Transfer::Pq,
        Tf::AribStdB67 => Transfer::Hlg,
        _ => return None,
    };
    let pts = buffer.pts().map(|t| Duration::from_nanos(t.nseconds()));
    let buffer = buffer.into_mapped_buffer_readable().ok()?;
    let (width, height) = (info.width(), info.height());
    let plane = |i: usize| Some((*info.offset().get(i)?, u32::try_from(*info.stride().get(i)?).ok()?));
    let planes = [plane(0)?, plane(1)?, plane(2)?];
    let rows = [height, height.div_ceil(2), height.div_ceil(2)];
    let fits = planes
        .iter()
        .zip(rows)
        .all(|((at, stride), rows)| buffer.len() >= at + *stride as usize * rows as usize);
    let full = info.colorimetry().range() == VideoColorRange::Range0_255;
    let hdr = Hdr { transfer, planes, full };
    fits.then_some(Frame {
        width,
        height,
        stride: planes[0].1,
        pts,
        pixels: Pixels::Mapped(0, buffer),
        hdr: Some(hdr),
    })
}

impl Frame {
    pub fn planes(&self) -> Option<[(&[u8], u32); 3]> {
        let (hdr, data) = (self.hdr?, self.data());
        Some(hdr.planes.map(|(at, stride)| (&data[at..], stride)))
    }
}
