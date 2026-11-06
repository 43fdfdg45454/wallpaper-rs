use crate::probe::preroll;
use crate::{Error, Still, local_uri, make};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;
use std::path::Path;

pub const MAX_STRIP: usize = 16;

pub fn strip(path: &Path, count: usize, height: u32) -> Result<Vec<(f64, Still)>, Error> {
    crate::init()?;
    let sink = format!(
        "videoconvert ! videoscale ! videoflip video-direction=auto ! \
         appsink name=still caps=video/x-raw,format=RGBA,pixel-aspect-ratio=1/1,height={}",
        height.clamp(8, 512)
    );
    let playbin = make("playbin")?;
    let bin =
        gst::parse::bin_from_description(&sink, true).map_err(|_| Error::MissingElement("videoscale"))?;
    let appsink = bin.by_name("still").and_then(|s| s.downcast::<gst_app::AppSink>().ok());
    let appsink = appsink.ok_or(Error::MissingElement("appsink"))?;
    playbin.set_property("uri", local_uri(path)?);
    playbin.set_property_from_str("flags", "video");
    playbin.set_property("video-sink", &bin);
    let frames = grab(&playbin, &appsink, count.clamp(1, MAX_STRIP));
    let _ = playbin.set_state(gst::State::Null);
    frames
}

fn grab(playbin: &gst::Element, sink: &gst_app::AppSink, count: usize) -> Result<Vec<(f64, Still)>, Error> {
    preroll(playbin)?;
    let duration = playbin.query_duration::<gst::ClockTime>().filter(|d| !d.is_zero());
    let length = duration.map_or(0.0, |d| d.nseconds() as f64 / 1e9);
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let at = length * (i as f64 + 0.5) / count as f64;
        let flags = gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE;
        if duration.is_some() {
            playbin
                .seek_simple(flags, gst::ClockTime::from_nseconds((at * 1e9) as u64))
                .map_err(|e| Error::Decode(e.to_string()))?;
            preroll(playbin)?;
        }
        out.push((at, frame(sink)?));
        if duration.is_none() {
            break;
        }
    }
    Ok(out)
}

fn frame(sink: &gst_app::AppSink) -> Result<Still, Error> {
    let no_frame = || Error::Decode("the file has no video frame".into());
    let sample = sink.pull_preroll().map_err(|_| no_frame())?;
    let info = gst_video::VideoInfo::from_caps(sample.caps().ok_or_else(no_frame)?)
        .map_err(|e| Error::Decode(e.to_string()))?;
    let buffer = sample.buffer().ok_or_else(no_frame)?.map_readable().map_err(|_| no_frame())?;
    let (width, height, stride) = (info.width(), info.height(), info.stride()[0] as usize);
    let row = width as usize * 4;
    let rows =
        buffer.get(info.offset()[0]..).unwrap_or_default().chunks(stride.max(row)).take(height as usize);
    let rgba: Vec<u8> = rows.flat_map(|r| r.get(..row).unwrap_or_default().iter().copied()).collect();
    if rgba.len() != row * height as usize {
        return Err(no_frame());
    }
    Ok(Still { width, height, duration_ms: None, audio: false, rgba })
}
