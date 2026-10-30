use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use wprs_scene::{Playlist, Voice};
use wprs_video::{Bank, Mix, RATE, Soundtrack, Source, Track, decode, probe_sound};

const ON: Voice = Voice { play: true, volume: 1.0, pan: 0.0, at: 0.0 };
const SECOND: f64 = RATE as f64;

fn track(lengths: &[f64], repeat: bool) -> Track {
    let list = Playlist { lengths: lengths.to_vec(), repeat, ..Playlist::default() };
    Track { list, fade_in: 0.0, fade_out: 0.02 }
}

fn run(mix: &mut Mix, frames: usize) -> Vec<[i16; 2]> {
    let mut out = vec![0u8; frames * 4];
    for chunk in out.chunks_mut(480 * 4) {
        mix.fill(chunk);
    }
    let pairs = out.as_chunks::<4>().0.iter();
    pairs.map(|f| [i16::from_le_bytes([f[0], f[1]]), i16::from_le_bytes([f[2], f[3]])]).collect()
}

fn wav(dir: &Path, seconds: u32) -> PathBuf {
    let bytes = RATE * 2 * seconds;
    let head = [&b"RIFF"[..], &(36 + bytes).to_le_bytes(), b"WAVEfmt ", &16u32.to_le_bytes(), &[1, 0, 1, 0]];
    let format =
        [&RATE.to_le_bytes()[..], &(RATE * 2).to_le_bytes(), &[2, 0, 16, 0], b"data", &bytes.to_le_bytes()];
    let tone = (0..RATE * seconds).flat_map(|i| ((i / 8) as i16).to_le_bytes());
    let path = dir.join(format!("ramp-{seconds}.wav"));
    std::fs::write(&path, [head.concat(), format.concat(), tone.collect()].concat()).unwrap();
    path
}

fn mp3(dir: &Path) -> Option<PathBuf> {
    use gstreamer::prelude::*;
    gstreamer::init().ok()?;
    let path = dir.join("tone.mp3");
    let line = format!(
        "audiotestsrc num-buffers=100 samplesperbuffer=1152 ! audio/x-raw,rate=48000,channels=1 ! audioconvert ! lamemp3enc target=bitrate cbr=true bitrate=96 ! filesink location={}",
        path.display()
    );
    let pipeline = gstreamer::parse::launch(&line).ok()?;
    pipeline.set_state(gstreamer::State::Playing).ok()?;
    let bus = pipeline.bus()?;
    bus.timed_pop_filtered(gstreamer::ClockTime::from_seconds(10), &[gstreamer::MessageType::Eos])?;
    pipeline.set_state(gstreamer::State::Null).ok()?;
    Some(path)
}

fn required(e: impl std::fmt::Display) {
    assert!(std::env::var_os("WPRS_REQUIRE_VIDEO").is_none(), "sound required: {e}");
}

fn wait(done: impl FnMut() -> bool) -> bool {
    within(Duration::from_secs(10), done).is_some()
}

fn within(limit: Duration, mut done: impl FnMut() -> bool) -> Option<Duration> {
    let start = Instant::now();
    while !done() {
        if start.elapsed() > limit {
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Some(start.elapsed())
}

#[test]
fn sounds_decode_from_wav_and_mp3_and_long_ones_stream() {
    let dir = tempfile::tempdir().unwrap();
    let file = wav(dir.path(), 3);
    let clip = match decode(&file, usize::MAX) {
        Ok(clip) => clip,
        Err(e) => return required(e),
    };
    assert_eq!((clip.channels, clip.frames()), (1, 3 * RATE as usize));
    assert!(decode(&dir.path().join("missing.ogg"), usize::MAX).is_err());
    let over = decode(&file, RATE as usize / 2).unwrap_err().to_string();
    assert!(over.contains("longer than 0.5 s"), "the decoded length is capped: {over}");
    assert_eq!(probe_sound(&file).unwrap(), 3.0);
    match mp3(dir.path()) {
        Some(mp3) => {
            let frames = decode(&mp3, usize::MAX).unwrap().frames() as f64 / SECOND;
            assert!((2.3..2.5).contains(&frames), "{frames}");
        }
        None => required("no MP3 encoder"),
    }
    let mut sound = Mix::new(&[track(&[3.0], true)]);
    sound.load(0, 0, Source::Stream(Arc::new(file)));
    sound.set(&[Voice { at: 2.0, ..ON }], 0.0);
    let mut heard = Vec::new();
    assert!(
        wait(|| {
            heard = run(&mut sound, 480);
            heard[479][0] != 0
        }),
        "the stream never played"
    );
    let at = sound.playheads()[0] as i64 - 1;
    assert!((i64::from(heard[479][0]) - at / 8).abs() <= 2, "{at} {:?}", heard[479]);
    sound.set(&[Voice { at: 0.5, ..ON }], sound.frames as f64);
    assert!(
        wait(|| run(&mut sound, 480)[479][0] < 4000 && run(&mut sound, 480)[479][0] > 0),
        "it did not seek back"
    );
}

#[test]
fn soundtracks_stay_near_the_speaker_and_pause_and_resume_promptly() {
    let dir = tempfile::tempdir().unwrap();
    let bank = Bank::new(vec![(wav(dir.path(), 1), false)]);
    let sound = match Soundtrack::open(&bank, &[(vec![Some(0)], track(&[1.0], true))], &[ON], 0.0, |_| {}) {
        Ok(sound) => sound,
        Err(e) => return required(e),
    };
    sound.set_playing(true);
    let position = || sound.position().unwrap_or_default();
    assert!(wait(|| position() > Duration::from_millis(300)), "the soundtrack did not start");
    let mut leads = Vec::new();
    let settled = within(Duration::from_secs(1), || {
        leads.push(sound.latency().unwrap_or(Duration::MAX));
        leads.len() >= 5 && median(&leads[leads.len() - 5..]) < Duration::from_millis(100)
    });
    assert!(settled.is_some(), "the soundtrack decodes too far ahead: {leads:?}");
    let asked = Instant::now();
    sound.set_playing(false);
    let (mut last, mut moved) = ((position(), sound.playheads().0), asked);
    let stopped = within(Duration::from_secs(2), || {
        std::thread::sleep(Duration::from_millis(30));
        let now = (position(), sound.playheads().0);
        if std::mem::replace(&mut last, now) != now {
            moved = Instant::now();
        }
        moved.elapsed() >= Duration::from_millis(300)
    });
    let took = moved - asked;
    assert!(stopped.is_some() && took < Duration::from_millis(500), "the pause took {took:?}");
    let (paused, frames) = last;
    let crept = position().saturating_sub(paused);
    assert!(crept < Duration::from_millis(20) && sound.playheads().0 == frames, "it kept playing: {crept:?}");
    let resumed = Instant::now();
    sound.set_playing(true);
    let played = within(Duration::from_secs(2), || position() > paused + Duration::from_millis(300));
    let late = played.map(|_| resumed.elapsed().saturating_sub(Duration::from_millis(300)));
    assert!(late.is_some_and(|late| late < Duration::from_millis(700)), "it resumed {late:?} late");
    let (made, heads) = sound.playheads();
    assert!((heads[0] - made as f64).abs() < 1.0, "the lane kept the scene time: {heads:?} {made}");
}

fn median(values: &[Duration]) -> Duration {
    let mut sorted = values.to_vec();
    sorted.sort();
    sorted[sorted.len() / 2]
}
