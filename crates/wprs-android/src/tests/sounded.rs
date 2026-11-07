use super::*;
use crate::sound::{Gate, Sounds, Want, decide, report, scale};
use crate::{props, settings};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use wprs_core::settings::Playback;
use wprs_scene::Runtime;

const ON: Gate = Gate { silent: false, shown: true, running: true };

fn wav() -> Vec<u8> {
    let samples: Vec<i16> = (0..24_000).map(|i| if i / 50 % 2 == 0 { 8000 } else { -8000 }).collect();
    let mut bytes = b"RIFF".to_vec();
    bytes.extend((36 + samples.len() as u32 * 2).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend([16u32.to_le_bytes().as_slice(), &1u16.to_le_bytes(), &1u16.to_le_bytes()].concat());
    bytes
        .extend([48_000u32.to_le_bytes().as_slice(), &96_000u32.to_le_bytes(), &2u16.to_le_bytes()].concat());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend((samples.len() as u32 * 2).to_le_bytes());
    bytes.extend(samples.iter().flat_map(|s| s.to_le_bytes()));
    bytes
}

fn packaged(id: &str, repeat: bool) -> PathBuf {
    let source = tempfile::tempdir().unwrap();
    fs::write(source.path().join("tone.wav"), wav()).unwrap();
    let layer = json!({ "id": "bell", "type": "sound", "sound": "tone", "loop": repeat });
    let text = json!({ "schema": wprs_scene::SCHEMA, "canvas": { "size": [64, 32] },
        "assets": { "tone": { "path": "tone.wav" } }, "layers": [layer] });
    fs::write(source.path().join("scene.json"), text.to_string()).unwrap();
    let manifest = json!({ "schema": wprs_format::SCHEMA, "id": id, "version": "1.0.0", "title": "Tone",
        "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "content": { "rating": "general" },
        "photosensitive": false, "origin": { "kind": "original" } });
    fs::write(source.path().join("manifest.json"), manifest.to_string()).unwrap();
    let built = source.path().join("tone.wpak");
    wprs_format::build(source.path(), &built, &wprs_format::Limits::default()).unwrap();
    let uid = store::store(&mut File::open(&built).unwrap(), "tone.wpak").unwrap().0;
    library::with(|lib| Ok(lib.item(&uid)?.path)).unwrap()
}

fn runtime(pack: &crate::pack::Pack) -> Runtime {
    let mut runtime = Runtime::new(pack.scene.clone(), props::values(pack));
    runtime.set_lengths(pack.sounds.iter().map(|(id, _, seconds)| (id.clone(), *seconds)).collect());
    runtime
}

fn heard(sounds: &Sounds) -> bool {
    (0..500).any(|_| {
        std::thread::sleep(Duration::from_millis(10));
        sounds.heard(480).iter().any(|frame| frame[0] != 0)
    })
}

#[test]
fn the_scene_sound_obeys_mute_volume_visibility_and_power() {
    let user = Playback { muted: false, volume: 0.5, ..Playback::default() };
    let off = Playback::default();
    assert_eq!(decide(&off, 100.0, ON), (Want::Off, 0.5), "scenes start muted");
    assert_eq!(decide(&user, 100.0, ON), (Want::On, 0.5));
    assert_eq!(decide(&user, 50.0, ON), (Want::On, 0.25));
    assert_eq!(decide(&user, 0.0, ON).0, Want::Off);
    assert_eq!(decide(&Playback { volume: 0.0, ..user.clone() }, 100.0, ON).0, Want::Off);
    assert_eq!(decide(&Playback { scene_sound: false, ..user.clone() }, 100.0, ON).0, Want::Off);
    assert_eq!(decide(&user, 100.0, Gate { silent: true, ..ON }).0, Want::Off, "lock screen and previews");
    assert_eq!(decide(&user, 100.0, Gate { shown: false, ..ON }).0, Want::Held, "not visible");
    assert_eq!(decide(&user, 100.0, Gate { running: false, ..ON }).0, Want::Held, "battery saver or heat");
}

#[test]
fn the_master_gain_glides_across_a_buffer() {
    let mut bytes: Vec<u8> = (0..400).flat_map(|_| 10_000i16.to_le_bytes()).collect();
    scale(&mut bytes, 1.0, 0.0);
    let frames: Vec<i16> = bytes.as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b)).collect();
    assert!(frames[0] >= 9_900 && frames[399] <= 100 && frames.windows(2).all(|w| w[0] >= w[1]));
}

pub(super) fn scene_sounds_play_pause_and_release() {
    let path = packaged("org.example.tone", true);
    let pack = pack::load(&path).unwrap();
    assert!(
        matches!(pack.sounds.as_slice(), [(id, file, seconds)] if id == "tone" && file.is_file() && *seconds == 0.5)
    );
    assert!(pack.properties.contains_key(wprs_scene::VOLUME), "a scene with sound has a volume setting");
    let (runtime, mut sounds, now) = (runtime(&pack), Sounds::new(pack.clone()), Instant::now());
    let before = report()["released"].as_u64().unwrap();
    assert_eq!(sounds.follow(&runtime, (Want::Off, 0.0), now), None);
    assert!(sounds.heard(8).is_empty() && !sounds.sounding());
    let wait = sounds.follow(&runtime, (Want::On, 0.5), now);
    assert!(
        wait.is_some_and(|w| w <= Duration::from_secs(1)),
        "{wait:?}: the sound is re-aligned every second"
    );
    assert!(sounds.sounding() && heard(&sounds), "a looping sound plays");
    assert_ne!(report()["session"], json!(-1));
    sounds.follow(&runtime, (Want::Held, 0.5), now);
    assert!(!sounds.sounding() && sounds.heard(8).is_empty(), "hidden or paused scenes release the output");
    sounds.follow(&runtime, (Want::On, 0.5), now);
    assert!(sounds.sounding() && sounds.heard(480).iter().any(|f| f[0] != 0), "decoded sounds are kept");
    sounds.follow(&runtime, (Want::Off, 0.0), now);
    assert_eq!(report()["released"].as_u64().unwrap(), before + 2);
    assert_eq!(report()["session"], json!(-1));
    ask(json!({ "op": "remove", "uids": [] }));
}

pub(super) fn a_one_shot_sound_pauses_when_it_ends_and_is_dropped_later() {
    let pack = pack::load(&packaged("org.example.once", false)).unwrap();
    let (mut runtime, mut sounds, now) = (runtime(&pack), Sounds::new(pack.clone()), Instant::now());
    sounds.follow(&runtime, (Want::On, 1.0), now);
    assert!(sounds.sounding() && heard(&sounds));
    runtime.advance(2.0);
    let wait = sounds.follow(&runtime, (Want::On, 1.0), now + Duration::from_secs(1));
    assert!(!sounds.sounding(), "an ended sound pauses its output");
    assert!(wait.is_some_and(|w| w <= Duration::from_secs(5)), "{wait:?}");
    sounds.follow(&runtime, (Want::On, 1.0), now + Duration::from_secs(8));
    assert!(sounds.heard(8).is_empty(), "five seconds later the output is released");
    settings::update(|config| {
        config.playback.muted = false;
        Ok(())
    })
    .unwrap();
    assert!(!settings::playback().muted);
    settings::update(|config| {
        config.playback.muted = true;
        Ok(())
    })
    .unwrap();
}
