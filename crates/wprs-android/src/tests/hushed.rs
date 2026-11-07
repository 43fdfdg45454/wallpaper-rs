use crate::hearing;
use crate::power::Power;
use crate::stage::Stage;
use std::time::{Duration, Instant};
use wprs_render::Renderer;
use wprs_scene::{Quality, Tier};

pub(super) fn silence_and_pace_changes_let_a_sound_scene_rest(stage: &mut Stage, r: &Renderer, power: Power) {
    let at = Instant::now();
    stage.pilot().start(Quality::Q0, Quality::Q3);
    for k in 1..=300u32 {
        hearing::hear(&[0i8; hearing::MAX_CAPTURE], 44_100);
        let when = at + Duration::from_millis(u64::from(k) * 17);
        stage.pilot().feed(when, 30.0, [16.7, 33.3], (Quality::Q3, true, true));
        let nanos = 1_000_000 + i64::from(k) * 17_000_000;
        let shot = stage.render(r, ((64, 128), nanos, 60), (power, Tier::T1), &[], false).unwrap();
        assert!(
            shot.image.is_none() && shot.delay == 0,
            "silence woke the scene at buffer {k}: {}",
            shot.delay
        );
    }
    assert!(!hearing::live() && stage.pilot().pace() == 2);
}
