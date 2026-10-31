use super::*;

fn turned(angle: f32) -> Reading {
    let (s, c) = (angle / 2.0).sin_cos();
    Reading::Turn([c, 0.0, s, 0.0])
}

#[test]
fn turning_or_tilting_wakes_the_wallpaper_and_stillness_does_not() {
    assert_eq!(
        [0, 1, 2, 3].map(|r| screen([1.0, 0.0, 0.0], r)),
        [[1.0, 0.0, 0.0], [-0.0, 1.0, 0.0], [-1.0, -0.0, 0.0], [0.0, -1.0, 0.0]]
    );
    let (start, mut s) = (Instant::now(), None);
    let at = |ms| start + Duration::from_millis(ms);
    assert!(feel(&mut s, turned(0.0), 0, start, 1.0));
    assert!(!feel(&mut s, turned(0.0), 0, at(20), 1.0));
    assert!(feel(&mut s, turned(0.1), 0, at(40), 1.0));
    let left = aimed(s.as_ref(), 0.5, at(50));
    assert!((left[0] + 0.4).abs() < 0.02 && left[1].abs() < 1e-3, "{left:?}");
    assert!(feel(&mut s, turned(0.1), 1, at(60), 1.0));
    let sideways = aimed(s.as_ref(), 0.5, at(70));
    assert!(sideways[0].abs() < 1e-3 && (sideways[1] + 0.4).abs() < 0.02, "{sideways:?}");
    assert_eq!(aimed(s.as_ref(), 0.75, at(2100)), [-0.5, 0.0]);
    assert!(feel(&mut s, Reading::Gravity([0.0, 0.8, 0.6]), 0, at(3000), 1.0), "a new reading starts over");
    assert!(feel(&mut s, Reading::Gravity([0.3, 0.75, 0.6]), 0, at(3020), 1.0));
    assert!(aimed(s.as_ref(), 0.5, at(3030))[0] > 0.5);
}

#[test]
fn late_samples_keep_the_rest_and_only_a_long_silence_starts_over() {
    let (start, mut state) = (Instant::now(), None);
    let at = |ms| start + Duration::from_millis(ms);
    let flat = || Reading::Gravity([0.0, 0.0, 1.0]);
    let leaning = || Reading::Gravity([0.1, 0.0, 0.995]);
    assert!(feel(&mut state, flat(), 0, start, 1.0));
    assert!(feel(&mut state, leaning(), 0, at(20), 1.0));
    let leant = aimed(state.as_ref(), 0.5, at(30));
    assert!(leant[0] > 0.35, "{leant:?}");
    assert!(!feel(&mut state, leaning(), 0, at(1020), 1.0), "a late sample keeps the picture where it was");
    assert_eq!(aimed(state.as_ref(), 0.5, at(1030)), leant);
    assert_eq!(aimed(state.as_ref(), 0.5, at(2500)), leant, "a short gap keeps the last aim");
    assert!(feel(&mut state, leaning(), 0, at(3100), 1.0), "a long silence starts over");
    assert_eq!(aimed(state.as_ref(), 0.5, at(3110)), [0.0, 0.0]);
}

#[test]
fn the_phone_shifts_a_share_of_what_it_shows_and_zooms_to_hide_the_edges() {
    let parallax = Parallax::default();
    assert_eq!(shift([0.5, -0.5], 0.02, 0.5, 1.0), [0.03, -0.015]);
    assert_eq!(shift([0.5, 0.0], 0.02, 1.0, 0.5), [0.015, 0.0]);
    assert_eq!(pixels(&parallax, (1080, 2400)), 0.02 * 3.0 * 1080.0);
    let reach = shift([1.0, 0.0], parallax.strength, 1.0, 0.45)[0] / 0.45 * 1080.0;
    assert!((pixels(&parallax, (1080, 2400)) - reach).abs() < 1e-3, "{reach}");
    let zoom = overscan(&parallax, 0.75, 1.0);
    assert!((zoom - 1.0 / (1.0 - 2.0 * 0.042)).abs() < 1e-4, "{zoom}");
    assert_eq!(overscan(&Parallax { strength: 0.0, ..parallax.clone() }, 1.0, 1.0), 1.04);
    assert!(overscan(&Parallax { strength: 0.03, focus: 0.0, ..parallax }, 4.0, 2.0) <= 1.0 / 0.6 + 1e-4);
}

#[test]
fn the_spring_settles_and_stops_asking_for_frames() {
    let mut motion = Motion::default();
    let steps: Vec<_> =
        (1..=60).map(|i| motion.step([0.5, -0.5], 150, (i * 33_000_000, 30), 144.0)).collect();
    assert!(steps[0].0 && steps[0].1);
    assert_eq!(steps.last(), Some(&(false, false)));
    assert!((motion.at()[0] - 0.5).abs() < 0.01);
    assert!(motion.rest() && !motion.rest());
}
