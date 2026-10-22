use crate::probe::preroll;
use crate::{Error, local_uri, make};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;
use std::path::Path;

const SINK: &str = "videoconvert ! videoscale ! videoflip video-direction=auto ! \
    appsink name=still caps=video/x-raw,format=RGBA,pixel-aspect-ratio=1/1";

#[derive(Debug, Clone, PartialEq)]
pub struct Still {
    pub width: u32,
    pub height: u32,
    pub duration_ms: Option<u32>,
    pub audio: bool,
    pub rgba: Vec<u8>,
}

pub fn still(path: &Path) -> Result<Still, Error> {
    crate::init()?;
    let uri = local_uri(path)?;
    let playbin = make("playbin")?;
    let bin =
        gst::parse::bin_from_description(SINK, true).map_err(|_| Error::MissingElement("videoconvert"))?;
    let sink = bin.by_name("still").and_then(|s| s.downcast::<gst_app::AppSink>().ok());
    let sink = sink.ok_or(Error::MissingElement("appsink"))?;
    playbin.set_property("uri", &uri);
    playbin.set_property_from_str("flags", "video+audio");
    playbin.set_property("audio-sink", make("fakesink")?);
    playbin.set_property("video-sink", &bin);
    let still = grab(&playbin, &sink);
    let _ = playbin.set_state(gst::State::Null);
    still
}

fn grab(playbin: &gst::Element, sink: &gst_app::AppSink) -> Result<Still, Error> {
    preroll(playbin)?;
    let duration = playbin.query_duration::<gst::ClockTime>().filter(|d| !d.is_zero());
    let audio = playbin.property::<i32>("n-audio") > 0;
    if let Some(duration) = duration {
        let at =
            (duration / 10).clamp(gst::ClockTime::SECOND, gst::ClockTime::from_seconds(30)).min(duration / 2);
        let flags = gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE;
        playbin.seek_simple(flags, at).map_err(|e| Error::Decode(e.to_string()))?;
        preroll(playbin)?;
    }
    let no_frame = || Error::Decode("the file has no video frame".into());
    let sample = sink.pull_preroll().map_err(|_| no_frame())?;
    let info = gst_video::VideoInfo::from_caps(sample.caps().ok_or_else(no_frame)?)
        .map_err(|e| Error::Decode(e.to_string()))?;
    let buffer = sample.buffer().ok_or_else(no_frame)?.map_readable().map_err(|_| no_frame())?;
    let (width, height, stride) = (info.width(), info.height(), info.stride()[0] as usize);
    let offset = info.offset()[0];
    let row = width as usize * 4;
    let rows = buffer.get(offset..).unwrap_or_default().chunks(stride.max(row)).take(height as usize);
    let rgba: Vec<u8> = rows.flat_map(|r| r.get(..row).unwrap_or_default().iter().copied()).collect();
    if rgba.len() != row * height as usize {
        return Err(no_frame());
    }
    let duration_ms = duration.and_then(|d| u32::try_from(d.mseconds()).ok());
    Ok(Still { width, height, duration_ms, audio, rgba })
}
