use gstreamer::prelude::*;
use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};
use wprs_video::{Event, Frame, Options, Player, is_video};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/magenta-64x32.webm");
const MAGENTA_RGB: [u8; 3] = [0xd0, 0x20, 0x60];
const LOSSY_TOLERANCE: u8 = 16;
const WAIT: Duration = Duration::from_secs(5);

fn play(path: &Path, max_size: (u32, u32)) -> Result<(Player, Receiver<Event>), wprs_video::Error> {
    let (tx, rx) = mpsc::channel();
    let options = Options { max_size, max_fps: 60, volume: 0.0, muted: true };
    Player::open(path, options, move |event| {
        let _ = tx.send(event);
    })
    .map(|player| (player, rx))
}

fn open(max_size: (u32, u32)) -> Option<(Player, Receiver<Event>)> {
    let started = play(Path::new(FIXTURE), max_size).map_err(|e| e.to_string()).and_then(|(player, rx)| {
        match rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Event::Frame(first)) => Ok((player, replay_first(first, rx))),
            Ok(Event::Error(e)) => Err(e),
            Err(_) => Err("no frame within 10 s".to_string()),
        }
    });
    if let Err(why) = &started {
        assert!(std::env::var_os("WPRS_REQUIRE_VIDEO").is_none(), "video playback required: {why}");
        eprintln!("skipping video test: {why}");
    }
    started.ok()
}

fn replay_first(first: Frame, rest: Receiver<Event>) -> Receiver<Event> {
    let (tx, rx) = mpsc::channel();
    tx.send(Event::Frame(first)).unwrap();
    std::thread::spawn(move || rest.into_iter().try_for_each(|event| tx.send(event).map_err(drop)));
    rx
}

fn next_frame(rx: &Receiver<Event>, timeout: Duration) -> Option<Frame> {
    match rx.recv_timeout(timeout) {
        Ok(Event::Frame(frame)) => Some(frame),
        Ok(Event::Error(e)) => panic!("playback error: {e}"),
        Err(_) => None,
    }
}

#[test]
fn detects_video_extensions() {
    assert!(is_video(Path::new("/a/b.MP4")));
    assert!(is_video(Path::new("clip.webm")));
    assert!(!is_video(Path::new("image.png")));
    assert!(!is_video(Path::new("noext")));
}

#[test]
fn decodes_rgba_frames() {
    let Some((_player, rx)) = open((1920, 1080)) else { return };
    let frame = next_frame(&rx, WAIT).expect("a frame");
    assert_eq!((frame.width, frame.height), (64, 32));
    assert!(frame.stride >= 64 * 4);
    let i = (16 * frame.stride + 32 * 4) as usize;
    let px = &frame.data()[i..i + 4];
    assert!(px.iter().zip(MAGENTA_RGB).all(|(&got, want)| got.abs_diff(want) <= LOSSY_TOLERANCE), "{px:?}");
    assert_eq!(px[3], 255);
}

#[test]
fn downscales_to_the_largest_output() {
    let Some((_player, rx)) = open((32, 32)) else { return };
    let frame = next_frame(&rx, WAIT).expect("a frame");
    assert_eq!((frame.width, frame.height), (32, 16));
}

#[test]
fn follows_the_largest_output_size() {
    let Some((player, rx)) = open((32, 32)) else { return };
    let size_within = |size, timeout: Duration| {
        let deadline = Instant::now() + timeout;
        std::iter::from_fn(|| (Instant::now() < deadline).then(|| next_frame(&rx, WAIT)).flatten())
            .any(|f| (f.width, f.height) == size)
    };
    assert!(size_within((32, 16), WAIT));
    player.set_max_size((1920, 1080));
    assert!(size_within((64, 32), WAIT), "no full-size frames after growing");
    player.set_max_size((16, 16));
    assert!(size_within((16, 8), WAIT), "no smaller frames after shrinking");
}

#[test]
fn loops_past_the_end() {
    let Some((_player, rx)) = open((64, 32)) else { return };
    let deadline = Instant::now() + Duration::from_millis(2500);
    let frames = std::iter::from_fn(|| {
        (Instant::now() < deadline).then(|| next_frame(&rx, Duration::from_secs(2))).flatten()
    })
    .count();
    assert!(frames > 30, "only {frames} frames in 2.5 s");
}

#[test]
fn a_lower_frame_rate_cap_applies_while_playing() {
    let Some((player, rx)) = open((64, 32)) else { return };
    let count = |span: Duration| {
        let deadline = Instant::now() + span;
        std::iter::from_fn(|| (Instant::now() < deadline).then(|| next_frame(&rx, WAIT)).flatten()).count()
    };
    let native = count(Duration::from_secs(1));
    player.set_max_fps(4);
    count(Duration::from_millis(500));
    let capped = count(Duration::from_secs(2));
    assert!(native > 12 && capped <= 12, "{native} frames in 1 s, then {capped} in 2 s at 4 FPS");
}

#[test]
fn pausing_stops_frames() {
    let Some((player, rx)) = open((64, 32)) else { return };
    assert!(next_frame(&rx, WAIT).is_some());
    player.set_playing(false);
    let deadline = Instant::now() + Duration::from_secs(30);
    while !player.paused() && Instant::now() < deadline {
        next_frame(&rx, Duration::from_millis(20));
    }
    assert!(player.paused(), "the pause did not settle within 30 s");
    let late = std::iter::from_fn(|| next_frame(&rx, Duration::from_millis(500))).take(10).count();
    assert!(late < 10 && next_frame(&rx, Duration::from_secs(1)).is_none(), "frames while paused");
    player.set_playing(true);
    assert!(next_frame(&rx, WAIT).is_some(), "no frames after resuming");
    assert!(!player.audio_enabled());
}

#[test]
fn unsupported_files_report_an_error() {
    if wprs_video::init().is_err() {
        return;
    }
    let text_file = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
    let Ok((_player, rx)) = play(text_file, (64, 32)) else { return };
    match rx.recv_timeout(Duration::from_secs(10)) {
        Ok(Event::Error(_)) => {}
        Ok(Event::Frame(_)) => panic!("a text file produced a frame"),
        Err(_) => panic!("no error reported"),
    }
}

#[test]
fn videos_with_sound_play_without_a_pipewire_audio_output() {
    if wprs_video::init().is_err() {
        return;
    }
    let clip = std::env::temp_dir().join(format!("wprs-sound-{}.webm", std::process::id()));
    let encode = format!(
        "videotestsrc num-buffers=30 ! video/x-raw,width=64,height=32,framerate=30/1 ! vp8enc ! webmmux name=m ! \
         filesink location={} audiotestsrc wave=silence num-buffers=50 ! audioconvert ! vorbisenc ! m.",
        clip.display()
    );
    let Ok(pipeline) = gstreamer::parse::launch(&encode) else { return };
    pipeline.set_state(gstreamer::State::Playing).unwrap();
    let done = pipeline.bus().unwrap().timed_pop_filtered(
        gstreamer::ClockTime::from_seconds(20),
        &[gstreamer::MessageType::Eos, gstreamer::MessageType::Error],
    );
    pipeline.set_state(gstreamer::State::Null).unwrap();
    assert!(
        done.is_some_and(|m| m.type_() == gstreamer::MessageType::Eos),
        "cannot encode a clip with sound"
    );
    let (tx, rx) = mpsc::channel();
    let options = Options { max_size: (64, 32), max_fps: 30, volume: 0.5, muted: false };
    let player = Player::open(&clip, options, move |event| drop(tx.send(event))).unwrap();
    let first = next_frame(&rx, Duration::from_secs(10));
    let deadline = Instant::now() + Duration::from_millis(2500);
    let frames =
        std::iter::from_fn(|| (Instant::now() < deadline).then(|| next_frame(&rx, WAIT)).flatten()).count();
    std::fs::remove_file(&clip).unwrap();
    assert!(first.is_some() && player.audio_enabled(), "no frame from a video with sound");
    assert!(frames > 45, "only {frames} frames in 2.5 s of a 1 s video with sound");
}
