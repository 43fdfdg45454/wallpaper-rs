use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};
use wprs_video::{Cut, Event, Options, Player};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/magenta-64x32.webm");
const FRAME: Duration = Duration::from_millis(60);

fn play(cut: Cut, max_fps: u32) -> Option<(Player, Receiver<Event>)> {
    let (tx, rx) = mpsc::channel();
    let options = Options { max_size: (64, 32), max_fps, volume: 0.0, muted: true, cut, zero_copy: false };
    let opened = Player::open(Path::new(FIXTURE), options, move |event| drop(tx.send(event)));
    match opened.map_err(|e| e.to_string()).and_then(|player| {
        match rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Event::Frame(_)) => Ok((player, rx)),
            Ok(Event::Error(e)) => Err(e),
            Err(_) => Err("no frame within 10 s".into()),
        }
    }) {
        Ok(playing) => Some(playing),
        Err(why) => {
            assert!(std::env::var_os("WPRS_REQUIRE_VIDEO").is_none(), "video playback required: {why}");
            None
        }
    }
}

fn frames(rx: &Receiver<Event>, skip: Duration, span: Duration) -> Vec<(Instant, Duration)> {
    let (start, mut seen) = (Instant::now(), Vec::new());
    while start.elapsed() < skip + span {
        match rx.recv_timeout(Duration::from_secs(2)) {
            Ok(Event::Frame(f)) if start.elapsed() >= skip => {
                seen.push((Instant::now(), f.pts.expect("a pts")))
            }
            Ok(Event::Frame(_)) => {}
            Ok(Event::Error(e)) => panic!("playback error: {e}"),
            Err(_) => break,
        }
    }
    seen
}

fn cpu() -> Duration {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap();
    let fields: Vec<&str> = stat.rsplit(')').next().unwrap().split_whitespace().collect();
    let ticks = |i: usize| fields[i].parse::<u64>().unwrap();
    Duration::from_millis((ticks(11) + ticks(12)) * 10)
}

#[test]
fn a_part_loops_within_its_range_at_its_speed() {
    for cut in [Cut { speed: 200, start: 100, end: 500 }, Cut { speed: 50, start: 300, end: 0 }] {
        let Some((_player, rx)) = play(cut, 60) else { return };
        let seen = frames(&rx, Duration::from_millis(400), Duration::from_secs(2));
        let end = Duration::from_millis(if cut.end == 0 { 1000 } else { cut.end.into() });
        let range = Duration::from_millis(cut.start.into()).saturating_sub(FRAME)..end + FRAME;
        assert!(seen.len() > 10, "{cut:?}: only {} frames", seen.len());
        assert!(seen.iter().all(|(_, pts)| range.contains(pts)), "{cut:?}: {seen:?}");
        let speed = speed(&seen);
        assert!((speed / f64::from(cut.speed) - 1.0).abs() < 0.35, "{cut:?} played at {speed:.0} %");
    }
}

#[test]
fn a_faster_video_draws_no_more_frames_than_the_cap() {
    for speed in [200, 400] {
        let Some((_player, rx)) = play(Cut { speed, ..Cut::default() }, 10) else { return };
        let seen = frames(&rx, Duration::from_millis(300), Duration::from_secs(2)).len();
        assert!((6..=24).contains(&seen), "{seen} frames in 2 s at {speed} % with a cap of 10 FPS");
    }
}

#[test]
fn a_start_at_or_past_the_end_plays_the_whole_video_without_spinning() {
    for start in [1000, 5000] {
        let Some((_player, rx)) = play(Cut { start, ..Cut::default() }, 30) else { return };
        let used = cpu();
        let seen = frames(&rx, Duration::ZERO, Duration::from_secs(2)).len();
        let used = cpu() - used;
        assert!(seen > 20, "only {seen} frames in 2 s with a start at {start} ms");
        assert!(used < Duration::from_millis(1500), "{used:?} of CPU in 2 s with a start at {start} ms");
    }
}

#[test]
fn a_new_part_applies_to_the_running_video() {
    let Some((mut player, rx)) = play(Cut::default(), 60) else { return };
    assert!(player.set_cut(Cut { speed: 100, start: 600, end: 0 }));
    let seen = frames(&rx, Duration::from_millis(500), Duration::from_secs(2));
    assert!(seen.len() > 10, "only {} frames after the change", seen.len());
    assert!(seen.iter().all(|(_, pts)| *pts + FRAME >= Duration::from_millis(600)), "{seen:?}");
}

fn speed(seen: &[(Instant, Duration)]) -> f64 {
    let steps = seen.windows(2).filter(|w| w[1].1 > w[0].1);
    let (stream, wall) = steps.fold((0.0, 0.0), |(s, w), p| {
        (s + (p[1].1 - p[0].1).as_secs_f64(), w + (p[1].0 - p[0].0).as_secs_f64())
    });
    stream / wall * 100.0
}

#[test]
fn a_new_speed_keeps_the_position_of_the_running_video() {
    let Some((mut player, rx)) = play(Cut { speed: 50, start: 0, end: 0 }, 60) else { return };
    let before = frames(&rx, Duration::from_millis(500), Duration::from_millis(200));
    let last = before.last().expect("frames before the change").1;
    assert!(player.set_cut(Cut { speed: 100, start: 0, end: 0 }));
    let after = frames(&rx, Duration::ZERO, Duration::from_millis(400));
    let first = after.first().expect("frames after the change").1;
    assert!(first + FRAME >= last, "the video went back from {last:?} to {first:?}");
    assert!((speed(&after) / 100.0 - 1.0).abs() < 0.35, "played at {:.0} %", speed(&after));
}

#[test]
fn a_seek_keeps_the_speed_and_the_end_of_the_part() {
    let cut = Cut { speed: 200, start: 100, end: 500 };
    let Some((player, rx)) = play(cut, 60) else { return };
    frames(&rx, Duration::from_millis(300), Duration::ZERO);
    player.seek(0.3);
    let seen = frames(&rx, Duration::from_millis(100), Duration::from_secs(2));
    let range = Duration::from_millis(100) - FRAME..Duration::from_millis(500) + FRAME;
    assert!(seen.len() > 10, "only {} frames after the seek", seen.len());
    assert!(seen.iter().all(|(_, pts)| range.contains(pts)), "{seen:?}");
    assert!((speed(&seen) / 200.0 - 1.0).abs() < 0.35, "played at {:.0} %", speed(&seen));
}
