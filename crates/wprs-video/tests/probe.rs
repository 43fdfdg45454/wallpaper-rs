use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use wprs_video::{Options, Player, probe};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/magenta-64x32.webm");

#[test]
fn probe_accepts_decodable_local_videos_only() {
    if wprs_video::init().is_err() {
        return;
    }
    match probe(Path::new(FIXTURE)) {
        Ok(size) => assert_eq!(size, (64, 32)),
        Err(e) => assert!(std::env::var_os("WPRS_REQUIRE_VIDEO").is_none(), "video probe required: {e}"),
    }
    let started = Instant::now();
    let garbage = std::env::temp_dir().join(format!("wprs-video-{}-garbage.webm", std::process::id()));
    std::fs::write(&garbage, [0x1a, 0x45, 0xdf, 0xa3, 0, 1, 2, 3].repeat(512)).unwrap();
    let rejected = probe(&garbage);
    let _ = std::fs::remove_file(&garbage);
    assert!(rejected.is_err(), "garbage accepted: {rejected:?}");
    assert!(probe(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))).is_err());
    for remote in ["http://127.0.0.1:9/clip.webm", "https://127.0.0.1:9/clip.webm", "rtsp://127.0.0.1:9/clip"]
    {
        assert!(matches!(probe(Path::new(remote)), Err(wprs_video::Error::NotLocal(_))), "{remote}");
        let (tx, _rx) = mpsc::channel();
        let options = Options { max_size: (64, 32), max_fps: 30, volume: 0.0, muted: true };
        assert!(Player::open(Path::new(remote), options, move |e| drop(tx.send(e))).is_err(), "{remote}");
    }
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[test]
fn no_element_can_fetch_remote_content() {
    if wprs_video::init().is_err() {
        return;
    }
    for uri in ["http://127.0.0.1:9/a.webm", "https://127.0.0.1:9/a.webm", "rtsp://127.0.0.1:9/a"] {
        assert!(gstreamer::Element::make_from_uri(gstreamer::URIType::Src, uri, None).is_err(), "{uri}");
    }
    for name in ["souphttpsrc", "hlsdemux", "hlsdemux2", "dashdemux", "dashdemux2", "rtspsrc", "sdpdemux"] {
        assert!(gstreamer::ElementFactory::find(name).is_none(), "{name} is still available");
    }
    let local = gstreamer::Element::make_from_uri(gstreamer::URIType::Src, "file:///dev/null", None);
    assert!(local.is_ok());
}

fn encode_rotated(path: &Path) -> Result<(), String> {
    use gstreamer::prelude::*;
    let description = format!(
        "videotestsrc num-buffers=15 ! video/x-raw,width=64,height=32,framerate=30/1 \
         ! taginject tags=image-orientation=rotate-90 scope=global ! jpegenc ! qtmux ! filesink location=\"{}\"",
        path.display()
    );
    let pipeline = gstreamer::parse::launch(&description).map_err(|e| e.to_string())?;
    pipeline.set_state(gstreamer::State::Playing).map_err(|e| e.to_string())?;
    let done = [gstreamer::MessageType::Eos, gstreamer::MessageType::Error];
    let message =
        pipeline.bus().and_then(|b| b.timed_pop_filtered(gstreamer::ClockTime::from_seconds(20), &done));
    let _ = pipeline.set_state(gstreamer::State::Null);
    match message.as_ref().map(|m| m.view()) {
        Some(gstreamer::MessageView::Eos(_)) => Ok(()),
        other => Err(format!("cannot encode the rotated clip: {other:?}")),
    }
}

#[test]
fn honours_rotation_tags() {
    if wprs_video::init().is_err() {
        return;
    }
    let path = std::env::temp_dir().join(format!("wprs-video-{}-rotated.mov", std::process::id()));
    if let Err(why) = encode_rotated(&path) {
        assert!(std::env::var_os("WPRS_REQUIRE_VIDEO").is_none(), "{why}");
        return eprintln!("skipping rotation test: {why}");
    }
    let (tx, rx) = mpsc::channel();
    let options = Options { max_size: (1920, 1080), max_fps: 30, volume: 0.0, muted: true };
    let player = Player::open(&path, options, move |e| drop(tx.send(e)));
    let first = rx.recv_timeout(Duration::from_secs(10));
    drop(player);
    let _ = std::fs::remove_file(&path);
    match first {
        Ok(wprs_video::Event::Frame(frame)) => assert_eq!((frame.width, frame.height), (32, 64)),
        other => panic!("no rotated frame: {other:?}"),
    }
}
