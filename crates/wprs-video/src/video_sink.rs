use crate::{Error, Event, Frame, Options, Sink, make};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;

pub fn build(options: Options, sink: Sink) -> Result<gst::Element, Error> {
    let bin = gst::Bin::with_name("wprs-video-sink");
    let rate = make("videorate")?;
    if options.max_fps > 0 {
        rate.set_property("max-rate", options.max_fps.min(i32::MAX as u32) as i32);
        if rate.find_property("drop-only").is_some() {
            rate.set_property("drop-only", true);
        }
    }
    let chain = [rate, make("videoconvert")?, make("videoscale")?, appsink(options.max_size, sink).upcast()];
    bin.add_many(&chain).map_err(Error::state)?;
    gst::Element::link_many(&chain).map_err(Error::state)?;
    let pad = chain[0].static_pad("sink").expect("videorate has a sink pad");
    let ghost = gst::GhostPad::with_target(&pad).map_err(Error::state)?;
    bin.add_pad(&ghost).map_err(Error::state)?;
    Ok(bin.upcast())
}

fn appsink((max_width, max_height): (u32, u32), sink: Sink) -> gst_app::AppSink {
    let up_to = |max: u32| gst::IntRange::new(1, max.clamp(1, i32::MAX as u32) as i32);
    let caps = gst::Caps::builder("video/x-raw")
        .field("format", "RGBA")
        .field("pixel-aspect-ratio", gst::Fraction::new(1, 1))
        .field("width", up_to(max_width))
        .field("height", up_to(max_height))
        .build();
    let appsink = gst_app::AppSink::builder()
        .caps(&caps)
        .sync(true)
        .max_buffers(1)
        .drop(true)
        .enable_last_sample(false)
        .build();
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
    let map = sample.buffer()?.map_readable().ok()?;
    let stride = u32::try_from(*info.stride().first()?).ok()?;
    let offset = *info.offset().first()?;
    let (width, height) = (info.width(), info.height());
    let len = stride as usize * height as usize;
    let data = map.as_slice().get(offset..offset + len)?.to_vec();
    Some(Frame { width, height, stride, data })
}
