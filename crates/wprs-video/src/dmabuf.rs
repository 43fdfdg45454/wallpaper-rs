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
    let Some(pad) = pad else { return 0 };
    let mut tags = (0..).map_while(|i| pad.sticky_event::<gst::event::Tag>(i));
    let named = tags.find_map(|t| t.tag().get::<gst::tags::ImageOrientation>().map(|v| v.get().to_owned()));
    named.and_then(|n| TURNS.iter().position(|t| *t == n)).unwrap_or(0) as u8
}

const SIGNS: [[i64; 4]; 8] = [
    [1, 0, 0, 1],
    [0, 1, -1, 0],
    [-1, 0, 0, -1],
    [0, -1, 1, 0],
    [-1, 0, 0, 1],
    [0, -1, -1, 0],
    [1, 0, 0, -1],
    [0, 1, 1, 0],
];

pub(crate) fn turned(
    rgba: &[u8],
    (width, height, stride): (u32, u32, usize),
    turn: u8,
) -> (u32, u32, Vec<u8>) {
    let [a, b, c, d] = SIGNS[usize::from(turn) % SIGNS.len()];
    let (w, h) = if turn % 2 == 1 { (height, width) } else { (width, height) };
    let mut out = Vec::with_capacity(w as usize * h as usize * 4);
    for y in 0..i64::from(h) {
        let dy = 2 * y + 1 - i64::from(h);
        for x in 0..i64::from(w) {
            let dx = 2 * x + 1 - i64::from(w);
            let sx = ((a * dx + b * dy + i64::from(width) - 1) / 2) as usize;
            let sy = ((c * dx + d * dy + i64::from(height) - 1) / 2) as usize;
            out.extend_from_slice(&rgba[sy * stride + sx * 4..][..4]);
        }
    }
    (w, h, out)
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

#[cfg(test)]
mod tests {
    use super::*;
    use gstreamer_app::AppSink;

    fn pull(pipeline: &gst::Bin, name: &str) -> Option<(u32, u32, usize, Vec<u8>)> {
        let sink = pipeline.by_name(name)?.downcast::<AppSink>().ok()?;
        let sample = sink.try_pull_sample(gst::ClockTime::from_seconds(10))?;
        let info = gst_video::VideoInfo::from_caps(sample.caps()?).ok()?;
        let map = sample.buffer()?.map_readable().ok()?;
        Some((info.width(), info.height(), usize::try_from(info.stride()[0]).ok()?, map.to_vec()))
    }

    #[test]
    fn copied_frames_turn_as_their_orientation_tag_says() {
        let source = "videotestsrc num-buffers=1 pattern=snow ! video/x-raw,format=RGBA,width=7,height=5";
        for (turn, name) in TURNS.iter().enumerate() {
            let line = format!(
                "{source} ! tee name=t t. ! queue ! appsink name=plain t. ! queue \
                 ! taginject tags=image-orientation={name} ! videoflip video-direction=auto ! appsink name=flipped"
            );
            let launched = crate::init().ok().and_then(|()| gst::parse::launch(&line).ok());
            let Some(pipeline) = launched.and_then(|p| p.downcast::<gst::Bin>().ok()) else {
                return assert!(std::env::var_os("WPRS_REQUIRE_VIDEO").is_none(), "videoflip required");
            };
            pipeline.set_state(gst::State::Playing).unwrap();
            let (plain, flipped) = (pull(&pipeline, "plain"), pull(&pipeline, "flipped"));
            pipeline.set_state(gst::State::Null).unwrap();
            let ((w, h, stride, rgba), (fw, fh, fstride, flipped)) = (plain.unwrap(), flipped.unwrap());
            let rows = flipped.chunks(fstride).take(fh as usize).flat_map(|r| &r[..fw as usize * 4]);
            let expected = (fw, fh, rows.copied().collect::<Vec<_>>());
            assert_eq!(turned(&rgba, (w, h, stride), turn as u8), expected, "{name}");
        }
    }
}
