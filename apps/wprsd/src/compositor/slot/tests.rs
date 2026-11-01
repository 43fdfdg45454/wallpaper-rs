use super::super::steer::Steer;
use super::*;
use wprs_video::Cut;

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/magenta-64x32.webm");
const FRAME: Duration = Duration::from_millis(60);

fn heard() -> (Settings, bool) {
    let (transition, max_fps, volume, muted) = (Duration::ZERO, 60, 1.0, false);
    let (software_parallax, flash_guard, scene_sound) = (false, false, true);
    (Settings { transition, max_fps, volume, muted, software_parallax, flash_guard, scene_sound }, false)
}

fn frames(slot: &PlayerSlot, span: Duration) -> Vec<(Instant, Duration)> {
    let (start, mut seen) = (Instant::now(), Vec::<(Instant, Duration)>::new());
    while start.elapsed() < span {
        let pts = lock(&slot.inbox.frame).take().and_then(|f| f.pts);
        let fresh = pts.filter(|pts| seen.last().is_none_or(|(_, last)| last != pts));
        fresh.into_iter().for_each(|pts| seen.push((Instant::now(), pts)));
        std::thread::sleep(Duration::from_millis(4));
    }
    seen
}

#[test]
fn a_steered_part_and_speed_reach_a_video_with_sound_opened_plain() {
    if let Err(e) = wprs_video::init() {
        return assert!(std::env::var_os("WPRS_REQUIRE_VIDEO").is_none(), "video required: {e}");
    }
    let (reel, (tx, _rx), now) =
        (Reel { path: FIXTURE.into(), cut: Cut::default() }, mpsc::channel(), Instant::now());
    let mut slot = PlayerSlot::new(((64, 32), true, 60, 100), now);
    slot.shares = false;
    slot.tick(&reel, heard(), &tx, now);
    assert!(slot.player.as_ref().is_some_and(Player::audio_enabled), "the video opened without sound");
    frames(&slot, Duration::from_millis(300));
    let cut = Cut { speed: 200, start: 300, end: 700 };
    slot.steer(Some(Steer { cut, jumps: 0, at: 0.0, wants: true }));
    assert_eq!(slot.steered, Some((cut, 0)));
    frames(&slot, Duration::from_millis(400));
    let seen = frames(&slot, Duration::from_secs(2));
    let range = Duration::from_millis(300) - FRAME..Duration::from_millis(700) + FRAME;
    assert!(seen.len() > 10, "only {} frames after steering", seen.len());
    assert!(seen.iter().all(|(_, pts)| range.contains(pts)), "{seen:?}");
    let steps = seen.windows(2).filter(|w| w[1].1 > w[0].1);
    let (stream, wall) = steps.fold((0.0, 0.0), |(s, w), p| {
        (s + (p[1].1 - p[0].1).as_secs_f64(), w + (p[1].0 - p[0].0).as_secs_f64())
    });
    assert!((stream / wall / 2.0 - 1.0).abs() < 0.35, "played at {:.0} %", stream / wall * 100.0);
}
