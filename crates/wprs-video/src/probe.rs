use crate::{Error, local_uri, make};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;
use std::path::Path;

const TIMEOUT: gst::ClockTime = gst::ClockTime::from_seconds(5);

pub fn probe(path: &Path) -> Result<(u32, u32), Error> {
    crate::init()?;
    let uri = local_uri(path)?;
    let playbin = make("playbin")?;
    let sink = gst_app::AppSink::builder().caps(&gst::Caps::new_empty_simple("video/x-raw")).build();
    playbin.set_property("uri", &uri);
    playbin.set_property_from_str("flags", "video");
    playbin.set_property("video-sink", &sink);
    let size = preroll(&playbin).and_then(|()| video_size(&sink));
    let _ = playbin.set_state(gst::State::Null);
    size
}

fn preroll(playbin: &gst::Element) -> Result<(), Error> {
    let started = playbin.set_state(gst::State::Paused).is_ok();
    match playbin.state(TIMEOUT).0 {
        Ok(gst::StateChangeSuccess::Async) if started => {
            Err(Error::Decode("timed out reading the file".into()))
        }
        Ok(_) if started => Ok(()),
        _ => Err(Error::Decode(bus_error(playbin))),
    }
}

fn bus_error(playbin: &gst::Element) -> String {
    let error = |bus: gst::Bus| bus.timed_pop_filtered(gst::ClockTime::SECOND, &[gst::MessageType::Error]);
    let message = playbin.bus().and_then(error);
    match message.as_ref().map(|m| m.view()) {
        Some(gst::MessageView::Error(e)) => e.error().to_string(),
        _ => "unsupported or damaged file".into(),
    }
}

fn video_size(sink: &gst_app::AppSink) -> Result<(u32, u32), Error> {
    let caps = sink.static_pad("sink").and_then(|pad| pad.current_caps());
    let caps = caps.ok_or_else(|| Error::Decode("the file has no video stream".into()))?;
    let info = gst_video::VideoInfo::from_caps(&caps).map_err(|e| Error::Decode(e.to_string()))?;
    Ok((info.width(), info.height()))
}
