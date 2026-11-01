use gstreamer::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};
use wprs_video::{Cut, Event, Options, Player};

const FRAME: Duration = Duration::from_millis(60);

fn clip(name: &str) -> Option<PathBuf> {
    wprs_video::init().ok()?;
    let clip = std::env::temp_dir().join(format!("wprs-heard-{name}-{}.webm", std::process::id()));
    let encode = format!(
        "videotestsrc num-buffers=60 ! video/x-raw,width=64,height=32,framerate=30/1 ! vp8enc ! webmmux name=m ! \
         filesink location={} audiotestsrc num-buffers=90 ! audioconvert ! vorbisenc ! m.",
        clip.display()
    );
    let pipeline = gstreamer::parse::launch(&encode).ok()?;
    pipeline.set_state(gstreamer::State::Playing).ok()?;
    let types = [gstreamer::MessageType::Eos, gstreamer::MessageType::Error];
    let done = pipeline.bus()?.timed_pop_filtered(gstreamer::ClockTime::from_seconds(20), &types);
    pipeline.set_state(gstreamer::State::Null).ok()?;
    done.filter(|m| m.type_() == gstreamer::MessageType::Eos).map(|_| clip)
}

fn play(path: &Path) -> (Player, Receiver<Event>) {
    let (tx, rx) = mpsc::channel();
    let (cut, zero_copy) = (Cut::default(), false);
    let options = Options { max_size: (64, 32), max_fps: 60, volume: 0.5, muted: false, cut, zero_copy };
    let player = Player::open(path, options, move |event| drop(tx.send(event))).unwrap();
    assert!(player.audio_enabled(), "the player has no sound");
    match rx.recv_timeout(Duration::from_secs(10)) {
        Ok(Event::Frame(_)) => (player, rx),
        Ok(Event::Error(e)) => panic!("playback error: {e}"),
        Err(_) => panic!("no frame within 10 s"),
    }
}

fn opened(name: &str) -> Option<(Player, Receiver<Event>, PathBuf)> {
    let Some(path) = clip(name) else {
        assert!(std::env::var_os("WPRS_REQUIRE_VIDEO").is_none(), "cannot encode a clip with sound");
        return None;
    };
    let (player, rx) = play(&path);
    Some((player, rx, path))
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

fn speed(seen: &[(Instant, Duration)]) -> f64 {
    let steps = seen.windows(2).filter(|w| w[1].1 > w[0].1);
    let (stream, wall) = steps.fold((0.0, 0.0), |(s, w), p| {
        (s + (p[1].1 - p[0].1).as_secs_f64(), w + (p[1].0 - p[0].0).as_secs_f64())
    });
    stream / wall * 100.0
}

#[test]
fn a_video_with_sound_takes_a_new_speed_and_part() {
    let Some((mut player, rx, path)) = opened("part") else { return };
    frames(&rx, Duration::from_millis(300), Duration::ZERO);
    player.set_cut(Cut { speed: 200, start: 300, end: 1500 });
    let seen = frames(&rx, Duration::from_millis(500), Duration::from_secs(2));
    std::fs::remove_file(path).unwrap();
    let range = Duration::from_millis(300) - FRAME..Duration::from_millis(1500) + FRAME;
    assert!(seen.len() > 10, "only {} frames after the change", seen.len());
    assert!(seen.iter().all(|(_, pts)| range.contains(pts)), "{seen:?}");
    assert!((speed(&seen) / 200.0 - 1.0).abs() < 0.35, "played at {:.0} %", speed(&seen));
}

#[test]
fn a_video_with_sound_keeps_its_position_on_a_new_speed() {
    let Some((mut player, rx, path)) = opened("speed") else { return };
    let before = frames(&rx, Duration::from_millis(300), Duration::from_millis(200));
    let last = before.last().expect("frames before the change").1;
    player.set_cut(Cut { speed: 50, start: 0, end: 0 });
    let after = frames(&rx, Duration::ZERO, Duration::from_millis(600));
    std::fs::remove_file(path).unwrap();
    let first = after.first().expect("frames after the change").1;
    assert!(first + FRAME >= last, "the video went back from {last:?} to {first:?}");
    assert!((speed(&after) / 50.0 - 1.0).abs() < 0.35, "played at {:.0} %", speed(&after));
}
