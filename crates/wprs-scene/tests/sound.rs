use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_scene::*;

fn props() -> BTreeMap<String, Property> {
    serde_json::from_str(include_str!("props.json")).unwrap()
}

fn scene(sounds: Json) -> Scene {
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [100, 50] }, "layers": sounds,
        "assets": { "wind": { "path": "wind.ogg" }, "pic": { "path": "pic.png" } },
        "timelines": [{ "id": "storm", "duration": 10, "loop": "repeat" }] });
    Scene::from_json(&text.to_string()).unwrap()
}

fn issues(layer: Json) -> Vec<Issue> {
    let mut layer = layer;
    layer["id"] = json!("s");
    layer["type"] = json!("sound");
    layer.as_object_mut().unwrap().entry("sound").or_insert(json!("wind"));
    scene(json!([layer])).validate(&props())
}

#[test]
fn sound_layers_validate_their_sources_and_ranges() {
    let good = json!({ "play": { "prop": "soft" }, "volume": { "timeline": "storm", "keys": [[0, 0.2], [5, 1]] },
                       "pan": -0.5, "loop": false, "fade_in": 2, "fade_out": 4 });
    assert_eq!(issues(good), []);
    let wrong = [
        (json!({ "volume": { "audio": "bass" } }), Issue::SoundSource("/layers/0/volume".into())),
        (json!({ "pan": { "cursor": "x" } }), Issue::SoundSource("/layers/0/pan".into())),
        (json!({ "play": { "script": "s.js" } }), Issue::SoundSource("/layers/0/play".into())),
        (json!({ "opacity": 0.5 }), Issue::Sound("/layers/0".into())),
        (json!({ "sound": "pic" }), Issue::Type("/layers/0/sound".into())),
        (
            json!({ "volume": 1.5 }),
            Issue::Range { path: "/layers/0/volume".into(), value: 1.5, min: 0.0, max: 1.0 },
        ),
        (
            json!({ "fade_out": 61 }),
            Issue::Range { path: "/layers/0/fade_out".into(), value: 61.0, min: 0.0, max: 60.0 },
        ),
    ];
    for (layer, issue) in wrong {
        assert!(issues(layer.clone()).contains(&issue), "{layer}: {:?}", issues(layer.clone()));
    }
    let many: Vec<Json> =
        (0..9).map(|i| json!({ "id": format!("s{i}"), "type": "sound", "sound": "wind" })).collect();
    assert_eq!(scene(json!(many)).validate(&props()), [Issue::Sounds(9)]);
    let mut odd = props();
    odd.insert(VOLUME.into(), serde_json::from_value(json!({ "type": "bool", "default": true })).unwrap());
    assert_eq!(
        scene(json!([{ "id": "s", "type": "sound", "sound": "wind" }])).validate(&odd),
        [Issue::Volume]
    );
    let masked = json!([{ "id": "s", "type": "sound", "sound": "wind" },
                        { "id": "m", "type": "solid", "color": "#ffffff", "mask": { "layer": "s" } }]);
    assert!(matches!(&scene(masked).validate(&props())[..], [Issue::Unknown { what: "layer", .. }]));
}

#[test]
fn scenes_with_sound_get_a_volume_property() {
    let quiet = scene(json!([]));
    let loud = scene(json!([{ "id": "s", "type": "sound", "sound": "wind" }]));
    assert!(!quiet.properties(&props(), "Volume").contains_key(VOLUME));
    let added = loud.properties(&props(), "Volume");
    assert_eq!((added[VOLUME].kind(), added[VOLUME].default_value()), ("slider", json!(100)));
}

#[test]
fn timelines_and_properties_start_and_stop_sounds() {
    let layer = json!([{ "id": "s", "type": "sound", "sound": "wind",
        "play": { "timeline": "storm", "keys": [[0, false], [2, true], [6, false]] },
        "volume": { "prop": "depth", "scale": 4 } }]);
    let mut runtime = Runtime::new(Arc::new(scene(layer)), effective(&props(), &[]));
    let mut at = |t: f64| {
        runtime.advance(t - runtime.time());
        runtime.voices()
    };
    let plays = [1.0, 3.0, 7.0, 13.0].map(|t| at(t).0[0].play);
    assert_eq!(plays, [false, true, false, true]);
    let (voices, wake) = at(14.0);
    assert_eq!((voices[0].volume, voices[0].pan, wake), (1.0, 0.0, Wake::Frame));
    let toggled = scene(json!([{ "id": "s", "type": "sound", "sound": "wind", "play": { "prop": "soft" } }]));
    let mut runtime = Runtime::new(Arc::new(toggled), effective(&props(), &[]));
    assert_eq!(runtime.voices(), (vec![Voice { play: true, volume: 1.0, pan: 0.0, at: 0.0 }], Wake::Idle));
    runtime.set_props(effective(&props(), &[&BTreeMap::from([("soft".into(), json!(false))])]));
    assert!(!runtime.voices().0[0].play);
    assert_eq!(runtime.frame([0.0, 0.0, 100.0, 50.0], Quality::Q3).draws.len(), 0);
}

fn wav(rate: u32, channels: u16, bytes: u32) -> Vec<u8> {
    let per_second = rate * u32::from(channels);
    let mut out =
        [&b"RIFF"[..], &(36 + bytes).to_le_bytes(), b"WAVEfmt ", &16u32.to_le_bytes(), &1u16.to_le_bytes()]
            .concat();
    out.extend(
        [
            &channels.to_le_bytes()[..],
            &rate.to_le_bytes(),
            &per_second.to_le_bytes(),
            &channels.to_le_bytes(),
            &8u16.to_le_bytes(),
        ]
        .concat(),
    );
    out.extend([&b"data"[..], &bytes.to_le_bytes()].concat());
    out.resize(out.len() + bytes as usize, 128);
    out
}

fn page(flags: u8, granule: u64, packet: &[u8]) -> Vec<u8> {
    let fixed = [&b"OggS\0"[..], &[flags], &granule.to_le_bytes(), &7u32.to_le_bytes(), &[0; 8]].concat();
    let lacing = if packet.is_empty() { vec![0] } else { vec![1, packet.len() as u8] };
    let mut page = [fixed, lacing, packet.to_vec()].concat();
    let crc = page.iter().fold(0u32, |crc, &b| {
        (0..8).fold(crc ^ u32::from(b) << 24, |c, _| if c >> 31 == 1 { c << 1 ^ 0x04c1_1db7 } else { c << 1 })
    });
    page[22..26].copy_from_slice(&crc.to_le_bytes());
    page
}

fn ogg(head: &[u8], last: u64) -> Vec<u8> {
    [page(2, 0, head), page(4, last, b"x")].concat()
}

#[test]
fn clips_report_their_length_and_long_ones_are_refused() {
    let seconds =
        |bytes: &[u8]| Clip::probe("a.ogg", bytes).map(|c| (c.channels, (c.seconds * 100.0).round() / 100.0));
    assert_eq!(seconds(&wav(8000, 1, 80_000)), Ok((1, 10.0)));
    assert!(seconds(&wav(8000, 1, 488_000)).unwrap_err().contains("at most 60 s"));
    assert!(seconds(&wav(8000, 6, 4800)).unwrap_err().contains("6 channels"));
    let info = (44_100u64 << 44) | (1 << 41) | (15 << 36) | 441_000;
    let flac = [&b"fLaC\0\0\0\x22"[..], &[0; 10], &info.to_be_bytes(), &[0; 16]].concat();
    assert_eq!(seconds(&flac), Ok((2, 10.0)));
    let vorbis = [&b"\x01vorbis\0\0\0\0\x01"[..], &22_050u32.to_le_bytes(), &[0; 14]].concat();
    assert_eq!(seconds(&ogg(&vorbis, 22_050 * 5)), Ok((1, 5.0)));
    let mut corrupt = page(4, 22_050, b"x");
    corrupt[22] ^= 1;
    let after = [page(0, 22_050, b"x"), corrupt, page(0, 22_050, b"")];
    let tails = after.map(|tail| seconds(&[ogg(&vorbis, 22_050 * 5), tail].concat()));
    assert!(tails.iter().all(|t| *t == Ok((1, 5.0))), "a later page or a wrong checksum counted: {tails:?}");
    assert_eq!(seconds(&[ogg(&vorbis, 22_050), page(4, 22_050 * 9, b"")].concat()), Ok((1, 9.0)));
    let opus = [&b"OpusHead\x01\x02"[..], &312u16.to_le_bytes(), &48_000u32.to_le_bytes(), &[0; 3]].concat();
    assert_eq!(seconds(&ogg(&opus, 48_000 * 3 + 312)), Ok((2, 3.0)));
    assert!(Clip::probe("a.mp3", b"ID3 not a supported sound").is_err());
    assert!(
        playable("sounds/Wind.OGG")
            && playable("a.wav")
            && playable("a.mp3")
            && !playable("a.aac")
            && !playable("ogg")
    );
}

#[test]
fn mp3_clips_report_their_length_and_long_ones_need_streaming() {
    let frame = |head: [u8; 4], size: usize| [&head[..], &vec![0; size - 4]].concat();
    let stereo = frame([0xff, 0xfb, 0x90, 0x00], 417);
    let cbr = [&b"ID3\x04\0\0\0\0\0\0"[..], &stereo.repeat(100)].concat();
    let clip = Clip::read("a.mp3", &cbr).unwrap();
    assert_eq!((clip.rate, clip.channels, (clip.seconds * 100.0).round() / 100.0), (44_100, 2, 2.61));
    let mono = frame([0xff, 0xf3, 0x80, 0xc0], 208).repeat(50);
    let clip = Clip::probe("a.mp3", &mono).unwrap();
    assert_eq!((clip.rate, clip.channels, (clip.seconds * 100.0).round() / 100.0), (22_050, 1, 1.31));
    let tagged = |frames: u32| {
        let mut first = stereo.clone();
        first[36..44].copy_from_slice(b"Xing\0\0\0\x01");
        first[44..48].copy_from_slice(&frames.to_be_bytes());
        [first, stereo.clone()].concat()
    };
    let long = Clip::read("a.mp3", &tagged(4000)).unwrap();
    assert_eq!((long.seconds * 10.0).round() / 10.0, 104.5);
    assert!(Clip::probe("a.mp3", &tagged(4000)).unwrap_err().contains("at most 60 s"));
    assert!(Clip::read("a.mp3", &tagged(40_000)).unwrap_err().contains("at most 600 s"));
    assert!(Clip::read("a.mp3", &frame([0xff, 0xfb, 0xf0, 0x00], 417)).unwrap_err().contains("MP3"));
}

#[test]
fn only_files_named_mp3_are_read_as_mp3_and_only_from_their_first_frame() {
    let frame = [&[0xff, 0xfb, 0x90, 0x00][..], &[0; 413]].concat();
    let song = frame.repeat(20);
    assert!(Clip::read("song.mp3", &song).is_ok());
    for name in ["song.ogg", "song.wav", "song"] {
        assert!(Clip::read(name, &song).unwrap_err().contains("named .mp3"), "{name}");
    }
    let noise: Vec<u8> = (0..5000u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8 & 0x7f).collect();
    let hidden = [&noise[..], &[0xff, 0xfb, 0x90, 0x00], &noise].concat();
    assert!(Clip::read("rain.mp3", &hidden).is_err());
    let lone = [&frame[..], &noise[..64]].concat();
    assert!(Clip::read("rain.mp3", &lone).is_err());
}

#[test]
fn streamed_clips_are_measured_from_their_edges() {
    let whole = wav(8000, 1, 800_000);
    let clip = Clip::sample("a.wav", &whole[..1024], &[], whole.len() as u64).unwrap();
    assert_eq!(clip.seconds, 100.0);
    let vorbis = [&b"\x01vorbis\0\0\0\0\x01"[..], &22_050u32.to_le_bytes(), &[0; 14]].concat();
    let long = [page(2, 0, &vorbis), vec![0; 4096], page(4, 22_050 * 200, b"x")].concat();
    let (head, tail) = (&long[..1024], &long[long.len() - 1024..]);
    assert_eq!(Clip::sample("a.ogg", head, tail, long.len() as u64).unwrap().seconds, 200.0);
    let stereo = [&[0xff, 0xfb, 0x90, 0x00][..], &[0; 413]].concat();
    let cbr = stereo.repeat(4000);
    let sampled = Clip::sample("a.mp3", &cbr[..4096], &[], cbr.len() as u64).unwrap();
    assert_eq!(sampled, Clip::read("a.mp3", &cbr).unwrap());
}
