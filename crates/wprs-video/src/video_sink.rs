use crate::{Error, Event, Frame, Options, Sink, make};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;

pub fn build(options: Options, sink: Sink) -> Result<(gst::Element, gst::Element, gst::Element), Error> {
    let bin = gst::Bin::with_name("wprs-video-sink");
    let rate = make("videorate")?;
    if options.max_fps > 0 {
        set_max_fps(&rate, options.max_fps);
        if rate.find_property("drop-only").is_some() {
            rate.set_property("drop-only", true);
        }
    }
    let convert = match make("videoconvertscale") {
        Ok(both) => vec![both],
        Err(_) => vec![make("videoconvert")?, make("videoscale")?],
    };
    let flip = make("videoflip").inspect(|f| f.set_property_from_str("video-direction", "auto"));
    let size = make("capsfilter")?;
    size.set_property("caps", size_caps(options.max_size));
    size.set_property_from_str("caps-change-mode", "delayed");
    let tail = [flip.ok(), Some(size.clone()), Some(appsink(sink).upcast())].into_iter().flatten();
    let chain: Vec<gst::Element> = std::iter::once(rate.clone()).chain(convert).chain(tail).collect();
    bin.add_many(&chain).map_err(Error::state)?;
    gst::Element::link_many(&chain).map_err(Error::state)?;
    let pad = chain[0].static_pad("sink").expect("videorate has a sink pad");
    let ghost = gst::GhostPad::with_target(&pad).map_err(Error::state)?;
    bin.add_pad(&ghost).map_err(Error::state)?;
    Ok((bin.upcast(), size, rate))
}

pub fn set_max_fps(rate: &gst::Element, max_fps: u32) {
    rate.set_property("max-rate", max_fps.clamp(1, i32::MAX as u32) as i32);
}

pub fn size_caps((max_width, max_height): (u32, u32)) -> gst::Caps {
    let up_to = |max: u32| gst::IntRange::new(1, max.clamp(1, i32::MAX as u32) as i32);
    gst::Caps::builder("video/x-raw")
        .field("format", "RGBA")
        .field("pixel-aspect-ratio", gst::Fraction::new(1, 1))
        .field("width", up_to(max_width))
        .field("height", up_to(max_height))
        .build()
}

fn appsink(sink: Sink) -> gst_app::AppSink {
    let appsink =
        gst_app::AppSink::builder().sync(true).max_buffers(1).drop(true).enable_last_sample(false).build();
    let preroll_sink = sink.clone();
    appsink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |appsink| deliver(appsink.pull_sample().ok(), &sink))
            .new_preroll(move |appsink| deliver(appsink.pull_preroll().ok(), &preroll_sink))
            .build(),
    );
    appsink
}

fn deliver(sample: Option<gst::Sample>, sink: &Sink) -> Result<gst::FlowSuccess, gst::FlowError> {
    if let Some(frame) = to_frame(&sample.ok_or(gst::FlowError::Eos)?) {
        sink(Event::Frame(frame));
    }
    Ok(gst::FlowSuccess::Ok)
}

fn to_frame(sample: &gst::Sample) -> Option<Frame> {
    let info = gst_video::VideoInfo::from_caps(sample.caps()?).ok()?;
    let buffer = sample.buffer_owned()?.into_mapped_buffer_readable().ok()?;
    let stride = u32::try_from(*info.stride().first()?).ok()?;
    let offset = *info.offset().first()?;
    let (width, height) = (info.width(), info.height());
    let complete = buffer.len() >= offset + stride as usize * height as usize;
    complete.then_some(Frame { width, height, stride, offset, buffer })
}
