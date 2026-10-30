use crate::{Cut, Error, Event, Frame, Options, Pixels, Sink, dmabuf, make};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

pub fn build(
    options: Options,
    sink: Sink,
    seen: Arc<AtomicU64>,
) -> Result<(gst::Element, gst::Element, gst::Element, bool), Error> {
    let bin = gst::Bin::with_name("wprs-video-sink");
    let rate = make("videorate")?;
    if options.max_fps > 0 {
        set_max_fps(&rate, options.max_fps, options.cut);
        if rate.find_property("drop-only").is_some() {
            rate.set_property("drop-only", true);
        }
    }
    let convert = match make("videoconvertscale") {
        Ok(both) => vec![both],
        Err(_) => vec![make("videoconvert")?, make("videoscale")?],
    };
    let flip = make("videoflip").inspect(|f| f.set_property_from_str("video-direction", "auto"));
    let shares = options.zero_copy && dmabuf::caps().is_some();
    let flip = flip.ok().filter(|_| !shares);
    let size = make("capsfilter")?;
    size.set_property("caps", size_caps(options.max_size, shares));
    size.set_property_from_str("caps-change-mode", "delayed");
    let tail = [flip, Some(size.clone()), Some(appsink(sink, shares).upcast())].into_iter().flatten();
    let chain: Vec<gst::Element> = std::iter::once(rate.clone()).chain(convert).chain(tail).collect();
    bin.add_many(&chain).map_err(Error::state)?;
    gst::Element::link_many(&chain).map_err(Error::state)?;
    let pad = chain[0].static_pad("sink").expect("videorate has a sink pad");
    pad.add_probe(gst::PadProbeType::BUFFER, move |_, _| {
        seen.fetch_add(1, Ordering::Relaxed);
        gst::PadProbeReturn::Ok
    });
    let ghost = gst::GhostPad::with_target(&pad).map_err(Error::state)?;
    bin.add_pad(&ghost).map_err(Error::state)?;
    Ok((bin.upcast(), size, rate, shares))
}

pub fn set_max_fps(rate: &gst::Element, max_fps: u32, cut: Cut) {
    rate.set_property("max-rate", cut.cap(max_fps));
}

pub fn size_caps((max_width, max_height): (u32, u32), zero_copy: bool) -> gst::Caps {
    let up_to = |max: u32| gst::IntRange::new(1, max.clamp(1, i32::MAX as u32) as i32);
    let copied = gst::Caps::builder("video/x-raw")
        .field("format", "RGBA")
        .field("pixel-aspect-ratio", gst::Fraction::new(1, 1))
        .field("width", up_to(max_width))
        .field("height", up_to(max_height))
        .build();
    let mut caps = dmabuf::caps().filter(|_| zero_copy).unwrap_or_else(gst::Caps::new_empty);
    caps.merge(copied);
    caps
}

fn appsink(sink: Sink, shares: bool) -> gst_app::AppSink {
    let appsink =
        gst_app::AppSink::builder().sync(true).max_buffers(1).drop(true).enable_last_sample(false).build();
    let preroll_sink = sink.clone();
    appsink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |appsink| deliver((appsink, shares), appsink.pull_sample().ok(), &sink))
            .new_preroll(move |appsink| {
                deliver((appsink, shares), appsink.pull_preroll().ok(), &preroll_sink)
            })
            .build(),
    );
    let pad = appsink.static_pad("sink").filter(|_| shares);
    if let Some(pad) = pad {
        pad.add_probe(gst::PadProbeType::QUERY_DOWNSTREAM, metas);
    }
    appsink
}

fn metas(_: &gst::Pad, info: &mut gst::PadProbeInfo) -> gst::PadProbeReturn {
    if let Some(gst::QueryViewMut::Allocation(query)) = info.query_mut().map(|q| q.view_mut()) {
        query.add_allocation_meta::<gst_video::VideoMeta>(None);
    }
    gst::PadProbeReturn::Ok
}

fn deliver(
    (appsink, shares): (&gst_app::AppSink, bool),
    sample: Option<gst::Sample>,
    sink: &Sink,
) -> Result<gst::FlowSuccess, gst::FlowError> {
    let sample = sample.ok_or(gst::FlowError::Eos)?;
    let shared = sample.caps().and_then(|c| c.features(0)).is_some_and(|f| f.contains("memory:DMABuf"));
    let turn = if shares { dmabuf::turn(appsink.static_pad("sink")) } else { 0 };
    let frame = match shared {
        true => shared_frame(&sample, turn),
        false => to_frame(&sample, turn),
    };
    match frame {
        Some(frame) => sink(Event::Frame(frame)),
        None if shared => sink(Event::Error(crate::UNSHARED.into())),
        None => {}
    }
    Ok(gst::FlowSuccess::Ok)
}

fn shared_frame(sample: &gst::Sample, turn: u8) -> Option<Frame> {
    let (width, height, shared, buffer) = dmabuf::read(sample, turn)?;
    let pts = buffer.pts().map(|t| Duration::from_nanos(t.nseconds()));
    Some(Frame { width, height, stride: 0, pts, pixels: Pixels::Shared { shared, _held: buffer } })
}

fn to_frame(sample: &gst::Sample, turn: u8) -> Option<Frame> {
    let info = gst_video::VideoInfo::from_caps(sample.caps()?).ok()?;
    let buffer = sample.buffer_owned()?;
    let pts = buffer.pts().map(|t| Duration::from_nanos(t.nseconds()));
    let buffer = buffer.into_mapped_buffer_readable().ok()?;
    let stride = u32::try_from(*info.stride().first()?).ok()?;
    let offset = *info.offset().first()?;
    let (width, height) = (info.width(), info.height());
    if buffer.len() < offset + stride as usize * height as usize {
        return None;
    }
    if turn == 0 {
        return Some(Frame { width, height, stride, pts, pixels: Pixels::Mapped(offset, buffer) });
    }
    let (width, height, pixels) = dmabuf::turned(&buffer[offset..], (width, height, stride as usize), turn);
    let buffer = gst::Buffer::from_mut_slice(pixels).into_mapped_buffer_readable().ok()?;
    Some(Frame { width, height, stride: width * 4, pts, pixels: Pixels::Mapped(0, buffer) })
}
