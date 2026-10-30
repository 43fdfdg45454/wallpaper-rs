use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use wprs_video::{Cut, Options, Player, probe};

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
        let options = Options {
            max_size: (64, 32),
            max_fps: 30,
            volume: 0.0,
            muted: true,
            cut: Cut::default(),
            zero_copy: false,
        };
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
    wprs_video::accept_dmabuf(vec![0]);
    let firsts = [false, true].map(|zero_copy| {
        let (tx, rx) = mpsc::channel();
        let (max_size, cut) = ((1920, 1080), Cut::default());
        let options = Options { max_size, max_fps: 30, volume: 0.0, muted: true, cut, zero_copy };
        let player = Player::open(&path, options, move |e| drop(tx.send(e)));
        let first = rx.recv_timeout(Duration::from_secs(10));
        drop(player);
        first
    });
    let _ = std::fs::remove_file(&path);
    for (first, zero_copy) in firsts.into_iter().zip([false, true]) {
        let size = |f: &wprs_video::Frame| match f.dmabuf() {
            Some(shared) if shared.turn % 2 == 1 => (f.height, f.width),
            _ => (f.width, f.height),
        };
        match first {
            Ok(wprs_video::Event::Frame(frame)) => {
                assert_eq!(size(&frame), (32, 64), "zero copy {zero_copy}")
            }
            other => panic!("no rotated frame with zero copy {zero_copy}: {other:?}"),
        }
    }
}

#[test]
fn file_uris_decode_to_local_paths_only() {
    let path = |uri| wprs_video::path_from_uri(uri);
    assert_eq!(path("file:///tmp/a%20b%23c.webm"), Some("/tmp/a b#c.webm".into()));
    assert_eq!(path("file://localhost/tmp/x.png"), Some("/tmp/x.png".into()));
    assert_eq!(path("file://elsewhere/tmp/x.png"), None);
    assert_eq!(path("http://127.0.0.1:9/x.webm"), None);
    assert_eq!(path("/tmp/x.png"), None);
}

#[test]
fn stills_come_from_inside_the_video() {
    if wprs_video::init().is_err() {
        return;
    }
    let still = match wprs_video::still(Path::new(FIXTURE)) {
        Ok(still) => still,
        Err(e) => return assert!(std::env::var_os("WPRS_REQUIRE_VIDEO").is_none(), "still required: {e}"),
    };
    assert_eq!((still.width, still.height, still.audio), (64, 32, false));
    assert!(still.duration_ms.is_some_and(|ms| ms > 0), "{:?}", still.duration_ms);
    let center = &still.rgba[(16 * 64 + 32) * 4..][..3];
    assert!(center[0] > 160 && center[1] < 80 && center[2] > 60, "{center:?}");
    assert!(wprs_video::still(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))).is_err());
}
