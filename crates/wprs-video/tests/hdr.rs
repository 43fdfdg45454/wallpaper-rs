use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};
use wprs_video::{Cut, Event, Frame, Options, Player, Transfer};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures");

fn first(name: &str) -> Option<Frame> {
    let (tx, rx) = mpsc::channel();
    let cut = Cut::default();
    let options =
        Options { max_size: (64, 32), max_fps: 30, volume: 0.0, muted: true, cut, zero_copy: false };
    let player = Player::open(&Path::new(FIXTURES).join(name), options, move |event| drop(tx.send(event)));
    if let Err(why) = &player {
        assert!(std::env::var_os("WPRS_REQUIRE_VIDEO").is_none(), "video playback required: {why}");
    }
    let player = player.ok()?;
    player.set_playing(true);
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Event::Frame(f)) => return Some(f),
            Ok(Event::Error(e)) => panic!("playback error: {e}"),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    panic!("no frame from {name}")
}

#[test]
fn hdr_videos_arrive_as_ten_bit_planes_and_others_as_rgba() {
    for (name, transfer) in [("pq-64x32.mkv", Transfer::Pq), ("hlg-64x32.mkv", Transfer::Hlg)] {
        let Some(frame) = first(name) else { return };
        let hdr = frame.hdr.unwrap_or_else(|| panic!("{name} came without its transfer"));
        assert_eq!((hdr.transfer, hdr.full, frame.width, frame.height), (transfer, false, 64, 32));
        let [luma, cb, cr] = frame.planes().unwrap();
        assert!(luma.1 >= 128 && cb.1 >= 64 && cr.1 >= 64 && cr.0.len() >= 16 * cr.1 as usize);
        let codes: Vec<u16> = luma.0[..128].chunks(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect();
        assert!(codes.iter().all(|c| *c <= 1023) && codes.iter().any(|c| *c > 255), "{codes:?}");
    }
    let Some(plain) = first("magenta-64x32.webm") else { return };
    assert!(plain.hdr.is_none() && plain.planes().is_none() && plain.stride >= 64 * 4);
}

fn ticks() -> u64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let fields: Vec<&str> = stat.rsplit(')').next().unwrap_or_default().split_whitespace().collect();
    fields
        .get(11)
        .zip(fields.get(12))
        .map_or(0, |(u, s)| u.parse::<u64>().unwrap_or(0) + s.parse::<u64>().unwrap_or(0))
}

#[test]
#[ignore]
fn the_cpu_a_clip_takes_to_play() {
    let Some(path) = std::env::var_os("WPRS_CLIP") else { return };
    let (tx, rx) = mpsc::channel();
    let options = Options {
        max_size: (3840, 2160),
        max_fps: 30,
        volume: 0.0,
        muted: true,
        cut: Cut::default(),
        zero_copy: false,
    };
    let player = Player::open(Path::new(&path), options, move |event| drop(tx.send(event))).unwrap();
    player.set_playing(true);
    let (start, before) = (Instant::now(), ticks());
    let (mut frames, mut bytes, mut deep) = (0, 0, false);
    while start.elapsed() < Duration::from_secs(10) {
        if let Ok(Event::Frame(f)) = rx.recv_timeout(Duration::from_millis(100)) {
            (frames, bytes, deep) = (frames + 1, f.data().len(), f.hdr.is_some());
        }
    }
    let cpu = (ticks() - before) as f64 / 100.0;
    eprintln!("{path:?}: {frames} frames, {deep}, {bytes} bytes a frame, {cpu:.2} s of CPU in 10 s");
}
