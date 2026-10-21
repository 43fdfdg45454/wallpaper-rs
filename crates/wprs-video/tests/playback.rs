//! Playback tests with a small VP8 fixture. They need the GStreamer base and
//! good plugins; set `WPRS_REQUIRE_VIDEO=1` to turn missing ones into failures.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use wprs_video::{Event, Frame, Options, Player};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/magenta-64x32.webm")
}

fn options(max_size: (u32, u32)) -> Options {
    Options {
        max_size,
        max_fps: 60,
        volume: 0.0,
        muted: true,
    }
}

/// Opens the fixture, or skips the test when GStreamer cannot play it.
fn open(opts: Options) -> Option<(Player, mpsc::Receiver<Event>)> {
    let (tx, rx) = mpsc::channel();
    let tx = std::sync::Mutex::new(tx);
    let player = Player::open(&fixture(), opts, move |e| {
        let _ = tx.lock().unwrap().send(e);
    });
    let skip = |why: String| {
        if std::env::var_os("WPRS_REQUIRE_VIDEO").is_some() {
            panic!("video playback required: {why}");
        }
        eprintln!("skipping video test: {why}");
    };
    match player {
        Ok(p) => match rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Event::Frame(f)) => {
                // Put the first frame back in front for the test.
                let (tx2, rx2) = mpsc::channel();
                tx2.send(Event::Frame(f)).unwrap();
                std::thread::spawn(move || {
                    for e in rx {
                        if tx2.send(e).is_err() {
                            break;
                        }
                    }
                });
                Some((p, rx2))
            }
            Ok(Event::Error(e)) => {
                skip(e);
                None
            }
            Err(_) => {
                skip("no frame within 10 s".into());
                None
            }
        },
        Err(e) => {
            skip(e.to_string());
            None
        }
    }
}

fn next_frame(rx: &mpsc::Receiver<Event>, timeout: Duration) -> Option<Frame> {
    match rx.recv_timeout(timeout) {
        Ok(Event::Frame(f)) => Some(f),
        Ok(Event::Error(e)) => panic!("playback error: {e}"),
        Err(_) => None,
    }
}

#[test]
fn decodes_rgba_frames() {
    let Some((_player, rx)) = open(options((1920, 1080))) else {
        return;
    };
    let f = next_frame(&rx, Duration::from_secs(5)).expect("a frame");
    assert_eq!((f.width, f.height), (64, 32));
    assert!(f.stride >= 64 * 4);
    let i = (16 * f.stride + 32 * 4) as usize;
    let px = &f.data[i..i + 4];
    // 0xd02060 after a lossy round trip.
    assert!(px[0].abs_diff(0xd0) <= 16, "{px:?}");
    assert!(px[1].abs_diff(0x20) <= 16, "{px:?}");
    assert!(px[2].abs_diff(0x60) <= 16, "{px:?}");
    assert_eq!(px[3], 255);
}

#[test]
fn downscales_to_the_largest_output() {
    let Some((_player, rx)) = open(options((32, 32))) else {
        return;
    };
    let f = next_frame(&rx, Duration::from_secs(5)).expect("a frame");
    assert_eq!((f.width, f.height), (32, 16));
}

#[test]
fn loops_past_the_end() {
    // The fixture lasts one second at 20 fps.
    let Some((_player, rx)) = open(options((64, 32))) else {
        return;
    };
    let start = Instant::now();
    let mut frames = 0;
    while start.elapsed() < Duration::from_millis(2500) {
        if next_frame(&rx, Duration::from_secs(2)).is_some() {
            frames += 1;
        } else {
            break;
        }
    }
    assert!(frames > 30, "only {frames} frames in 2.5 s");
}

#[test]
fn pausing_stops_frames() {
    let Some((player, rx)) = open(options((64, 32))) else {
        return;
    };
    assert!(next_frame(&rx, Duration::from_secs(5)).is_some());
    player.set_playing(false);
    std::thread::sleep(Duration::from_millis(300));
    while rx.try_recv().is_ok() {}
    assert!(
        next_frame(&rx, Duration::from_millis(600)).is_none(),
        "frames while paused"
    );
    player.set_playing(true);
    assert!(
        next_frame(&rx, Duration::from_secs(5)).is_some(),
        "no frames after resuming"
    );
    assert!(!player.audio_enabled());
}

#[test]
fn unsupported_files_report_an_error() {
    if wprs_video::init().is_err() {
        return;
    }
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let (tx, rx) = mpsc::channel();
    let tx = std::sync::Mutex::new(tx);
    let Ok(_player) = Player::open(&path, options((64, 32)), move |e| {
        let _ = tx.lock().unwrap().send(e);
    }) else {
        return;
    };
    match rx.recv_timeout(Duration::from_secs(10)) {
        Ok(Event::Error(_)) => {}
        Ok(Event::Frame(_)) => panic!("a text file produced a frame"),
        Err(_) => panic!("no error reported"),
    }
}
