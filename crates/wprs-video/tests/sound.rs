use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use wprs_scene::Voice;
use wprs_video::{Bank, Clip, Mix, RATE, Soundtrack, Track, decode};

const ON: Voice = Voice { play: true, volume: 1.0, pan: 0.0 };

fn mix(track: Track, clip: Clip) -> Mix {
    let mut mix = Mix::new(&[track]);
    mix.load(0, Arc::new(clip));
    mix
}

fn run(mix: &mut Mix, frames: usize) -> Vec<[i16; 2]> {
    let mut out = vec![0u8; frames * 4];
    mix.fill(&mut out);
    out.as_chunks::<4>()
        .0
        .iter()
        .map(|f| [i16::from_le_bytes([f[0], f[1]]), i16::from_le_bytes([f[2], f[3]])])
        .collect()
}

fn half(frames: usize) -> Clip {
    Clip { channels: 1, samples: vec![16_384; frames] }
}

#[test]
fn the_mix_fades_pans_loops_and_ends_one_shots() {
    let looped = Track { repeat: true, fade_in: 0.0, fade_out: 0.02 };
    let mut sound = mix(looped, half(480));
    assert!(run(&mut sound, 100).iter().all(|f| *f == [0, 0]), "silent until it plays");
    sound.set(&[ON]);
    assert_eq!(run(&mut sound, 1000)[999], [16_383, 16_383]);
    assert_eq!((sound.playheads(), sound.frames), (vec![1000 % 480], 1100));
    sound.set(&[Voice { pan: 1.0, ..ON }]);
    assert_eq!(run(&mut sound, 10)[9], [0, 16_383]);
    sound.set(&[Voice { play: false, ..ON }]);
    let tail = run(&mut sound, 1000);
    assert!(tail[0][0] > 8_000 && tail[500][0] < 8_000 && tail[970] == [0, 0], "fades out over 20 ms");
    assert_eq!(sound.playheads(), [0]);
    let mut shot = mix(Track::default(), Clip { channels: 2, samples: [100, -100].repeat(480) });
    shot.set(&[ON]);
    let first = run(&mut shot, 2000);
    assert!(first[479][0] > 0 && first[479][1] < 0 && first[480..].iter().all(|f| *f == [0, 0]));
    assert!(run(&mut shot, 100).iter().all(|f| *f == [0, 0]), "a one-shot stays ended while it plays");
    shot.set(&[Voice { play: false, ..ON }]);
    run(&mut shot, 1);
    shot.set(&[ON]);
    assert!(run(&mut shot, 10)[9][0] > 0, "and starts again once it was stopped");
}

fn wav(dir: &Path, seconds: u32) -> PathBuf {
    let bytes = RATE * 2 * seconds;
    let head = [&b"RIFF"[..], &(36 + bytes).to_le_bytes(), b"WAVEfmt ", &16u32.to_le_bytes(), &[1, 0, 1, 0]];
    let format =
        [&RATE.to_le_bytes()[..], &(RATE * 2).to_le_bytes(), &[2, 0, 16, 0], b"data", &bytes.to_le_bytes()];
    let tone = (0..RATE * seconds).flat_map(|i| (((i % 96) as i16 - 48) * 200).to_le_bytes());
    let path = dir.join(format!("tone-{seconds}.wav"));
    std::fs::write(&path, [head.concat(), format.concat(), tone.collect()].concat()).unwrap();
    path
}

fn wait(mut done: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    done()
}

#[test]
fn soundtracks_decode_and_keep_their_place_across_a_pause() {
    let dir = tempfile::tempdir().unwrap();
    let file = wav(dir.path(), 1);
    let clip = match decode(&file, usize::MAX) {
        Ok(clip) => clip,
        Err(e) => return assert!(std::env::var_os("WPRS_REQUIRE_VIDEO").is_none(), "sound required: {e}"),
    };
    assert_eq!((clip.channels, clip.frames()), (1, RATE as usize));
    assert!(decode(&dir.path().join("missing.ogg"), usize::MAX).is_err());
    let over = decode(&file, RATE as usize / 2).unwrap_err().to_string();
    assert!(over.contains("longer than 0.5 s"), "the decoded length is capped: {over}");
    let track = Track { repeat: true, ..Track::default() };
    let bank = Bank::new(vec![file]);
    let sound = Soundtrack::open(&bank, &[(Some(0), track)], &[ON], 0.0, |_| {}).unwrap();
    sound.set_playing(true);
    let position = || sound.position().unwrap_or_default();
    assert!(wait(|| position() > Duration::from_millis(100)), "the soundtrack did not start");
    assert!(wait(|| sound.playheads().1[0] > 0), "the clip did not load");
    let (start, first) = sound.playheads();
    std::thread::sleep(Duration::from_millis(500));
    sound.set_playing(false);
    std::thread::sleep(Duration::from_millis(300));
    let (paused, frames) = (position(), sound.playheads().0);
    std::thread::sleep(Duration::from_millis(600));
    assert!(
        position() - paused < Duration::from_millis(50) && sound.playheads().0 == frames,
        "it kept playing"
    );
    sound.set_playing(true);
    std::thread::sleep(Duration::from_millis(600));
    let (resumed, (made, heads)) = (position(), sound.playheads());
    let lead = made as f64 / f64::from(RATE) - resumed.as_secs_f64();
    assert!(
        (resumed - paused).as_secs_f64() > 0.4 && (0.0..0.3).contains(&lead),
        "{resumed:?} {paused:?} {lead}"
    );
    assert_eq!(heads, [((first[0] as u64 + made - start) % u64::from(RATE)) as usize]);
}
