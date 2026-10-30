use std::sync::Arc;
use wprs_scene::{Order, Playlist, Voice};
use wprs_video::{Clip, Mix, RATE, Source, Track};

const ON: Voice = Voice { play: true, volume: 1.0, pan: 0.0, at: 0.0 };
const SECOND: f64 = RATE as f64;

fn track(lengths: &[f64], repeat: bool) -> Track {
    let list = Playlist { lengths: lengths.to_vec(), repeat, ..Playlist::default() };
    Track { list, fade_in: 0.0, fade_out: 0.02 }
}

fn mix(track: Track, clips: Vec<Clip>) -> Mix {
    let mut mix = Mix::new(&[track]);
    clips.into_iter().enumerate().for_each(|(i, c)| mix.load(0, i, Source::Memory(Arc::new(c))));
    mix
}

fn run(mix: &mut Mix, frames: usize) -> Vec<[i16; 2]> {
    let mut out = vec![0u8; frames * 4];
    for chunk in out.chunks_mut(480 * 4) {
        mix.fill(chunk);
    }
    let pairs = out.as_chunks::<4>().0.iter();
    pairs.map(|f| [i16::from_le_bytes([f[0], f[1]]), i16::from_le_bytes([f[2], f[3]])]).collect()
}

fn flat(value: i16, frames: usize) -> Clip {
    Clip { channels: 1, samples: vec![value; frames] }
}

fn ramp(seconds: f64) -> Clip {
    Clip { channels: 1, samples: (0..(seconds * SECOND) as usize).map(|i| (i / 8) as i16).collect() }
}

#[test]
fn the_mix_fades_pans_loops_and_ends_one_shots() {
    let mut sound = mix(track(&[0.01], true), vec![flat(16_384, 480)]);
    assert!(run(&mut sound, 100).iter().all(|f| *f == [0, 0]), "silent until it plays");
    sound.set(&[ON], sound.frames as f64);
    assert_eq!(run(&mut sound, 1000)[999], [16_383, 16_383]);
    sound.set(&[Voice { pan: 1.0, ..ON }], sound.frames as f64);
    assert_eq!(run(&mut sound, 1000)[999], [0, 16_383], "pans glide");
    sound.set(&[Voice { play: false, ..ON }], 0.0);
    let tail = run(&mut sound, 1000);
    assert!(tail[0][1] > 8_000 && tail[500][1] < 8_000 && tail[970] == [0, 0], "fades out over 20 ms");
    let mut shot = mix(track(&[0.01], false), vec![Clip { channels: 2, samples: [100, -100].repeat(480) }]);
    shot.set(&[ON], 0.0);
    let first = run(&mut shot, 2000);
    assert!(first[479][0] > 0 && first[479][1] < 0 && first[480..].iter().all(|f| *f == [0, 0]));
    assert!(shot.quiet(), "an ended one-shot is quiet");
    shot.set(&[Voice { play: false, ..ON }], 0.0);
    run(&mut shot, 1);
    shot.set(&[ON], shot.frames as f64);
    assert!(run(&mut shot, 10)[9][0] > 0, "and starts again from where the scene says");
}

#[test]
fn lanes_follow_the_time_of_the_scene() {
    let mut sound = mix(track(&[1.0], true), vec![ramp(1.0)]);
    sound.set(&[ON], 0.0);
    run(&mut sound, 4800);
    assert_eq!(sound.playheads(), [4800.0]);
    sound.set(&[Voice { at: 0.12, ..ON }], 0.0);
    let heard = run(&mut sound, 2000);
    assert_eq!(sound.playheads(), [0.12 * SECOND + 2000.0 + 4800.0], "a late lane jumps to the scene");
    assert!((heard[1999][0] - ((0.12 * SECOND) as i16 + 6799) / 8).abs() <= 1, "{:?}", heard[1999]);
    let step = heard.windows(2).map(|w| (w[1][0] - w[0][0]).abs()).max();
    assert!(step < Some(16), "the jump fades out and back in: {step:?}");
    sound.set(&[Voice { at: 13_560.0 / SECOND, ..ON }], 6800.0);
    run(&mut sound, 10);
    assert_eq!(sound.playheads(), [12_570.0], "small drifts are left alone");
    sound.set(&[Voice { at: 2.25, ..ON }], sound.frames as f64);
    let looped = run(&mut sound, 1000);
    assert!(
        (looped[999][0] - (0.25 * SECOND + 999.0) as i16 / 8).abs() <= 1,
        "loops re-align: {:?}",
        looped[999]
    );
}

#[test]
fn lists_play_in_order_with_pauses_and_end() {
    let list =
        Playlist { lengths: vec![0.01, 0.02], pause: [0.01, 0.01], repeat: false, ..Playlist::default() };
    let track = Track { list, fade_in: 0.0, fade_out: 0.0 };
    let mut sound = mix(track, vec![flat(8_000, 480), flat(-8_000, 960)]);
    sound.set(&[ON], 0.0);
    let out = run(&mut sound, 3000);
    let at = |ms: usize| out[ms * 48 + 24][0].signum();
    assert_eq!([at(0), at(5), at(15), at(25), at(35), at(45)], [1, 1, 0, -1, -1, 0]);
    let random = Playlist {
        lengths: vec![0.01, 0.01],
        order: Order::Random,
        repeat: true,
        seed: 3,
        ..Playlist::default()
    };
    let mut sound =
        mix(Track { list: random, ..Track::default() }, vec![flat(8_000, 480), flat(-8_000, 480)]);
    sound.set(&[ON], 0.0);
    let out = run(&mut sound, 4800);
    let signs: Vec<i16> = (0..10).map(|i| out[i * 480 + 240][0].signum()).collect();
    assert!(signs.windows(2).all(|w| w[0] != w[1]), "{signs:?}");
}
