use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_allocators::DmaBufMemory;
use gstreamer_video as gst_video;
use std::os::fd::{BorrowedFd, RawFd};
use std::sync::OnceLock;

static MODIFIERS: OnceLock<Vec<u64>> = OnceLock::new();

const TURNS: [&str; 8] = [
    "rotate-0",
    "rotate-90",
    "rotate-180",
    "rotate-270",
    "flip-rotate-0",
    "flip-rotate-90",
    "flip-rotate-180",
    "flip-rotate-270",
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dmabuf {
    pub modifier: u64,
    planes: [(RawFd, u32, u32); 2],
    pub bt709: bool,
    pub full: bool,
    pub turn: u8,
}

impl Dmabuf {
    pub fn planes(&self) -> [(BorrowedFd<'_>, u32, u32); 2] {
        self.planes.map(|(fd, offset, stride)| (unsafe { BorrowedFd::borrow_raw(fd) }, offset, stride))
    }
}

pub fn accept(modifiers: Vec<u64>) {
    let _ = MODIFIERS.set(modifiers);
}

pub(crate) fn caps() -> Option<gst::Caps> {
    let modifiers = MODIFIERS.get().filter(|m| !m.is_empty())?;
    let name = |m: &u64| if *m == 0 { "NV12".to_owned() } else { format!("NV12:{m:#018x}") };
    let builder = gst::Caps::builder("video/x-raw").features(["memory:DMABuf"]).field("format", "DMA_DRM");
    Some(builder.field("drm-format", gst::List::new(modifiers.iter().map(name))).build())
}

pub(crate) fn turn(pad: Option<gst::Pad>) -> u8 {
    let tags = pad.and_then(|pad| pad.sticky_event::<gst::event::Tag>(0));
    let named =
        tags.as_ref().and_then(|t| t.tag().get::<gst::tags::ImageOrientation>()).map(|v| v.get().to_owned());
    named.and_then(|n| TURNS.iter().position(|t| *t == n)).unwrap_or(0) as u8
}

pub(crate) fn read(sample: &gst::Sample, turn: u8) -> Option<(u32, u32, Dmabuf, gst::Buffer)> {
    let caps = sample.caps()?;
    caps.features(0).filter(|f| f.contains("memory:DMABuf"))?;
    let s = caps.structure(0)?;
    let drm = s.get::<&str>("drm-format").ok()?;
    let (format, modifier) = drm.split_once(':').unwrap_or((drm, "0x0"));
    let modifier = u64::from_str_radix(modifier.trim_start_matches("0x"), 16).ok()?;
    let (width, height) = (s.get::<i32>("width").ok()?, s.get::<i32>("height").ok()?);
    (format == "NV12").then_some(())?;
    let buffer = sample.buffer_owned()?;
    let meta = buffer.meta::<gst_video::VideoMeta>()?;
    let plane = |i: usize| {
        let (offset, stride) = (*meta.offset().get(i)?, u32::try_from(*meta.stride().get(i)?).ok()?);
        let (found, skip) = buffer.find_memory(offset..offset + 1)?;
        let memory = buffer.peek_memory(found.start).downcast_memory_ref::<DmaBufMemory>()?;
        Some((memory.fd(), u32::try_from(memory.offset() + skip).ok()?, stride))
    };
    let planes = [plane(0)?, plane(1)?];
    let colors =
        s.get::<&str>("colorimetry").ok().and_then(|c| c.parse::<gst_video::VideoColorimetry>().ok());
    let bt709 = colors.as_ref().map_or(height >= 720, |c| c.matrix() == gst_video::VideoColorMatrix::Bt709);
    let full = colors.is_some_and(|c| c.range() == gst_video::VideoColorRange::Range0_255);
    let shared = Dmabuf { modifier, planes, bt709, full, turn };
    Some((u32::try_from(width).ok()?, u32::try_from(height).ok()?, shared, buffer))
}
