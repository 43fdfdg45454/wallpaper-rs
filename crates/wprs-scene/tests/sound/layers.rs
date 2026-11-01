use super::*;

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
    assert!(!quiet.properties(&props(), "Volume", 100.0).contains_key(VOLUME));
    let added = loud.properties(&props(), "Volume", 100.0);
    assert_eq!((added[VOLUME].kind(), added[VOLUME].default_value()), ("slider", json!(100)));
    let authored = loud.properties(&props(), "Volume", 40.0);
    assert_eq!(authored[VOLUME].default_value(), json!(40), "the author's playback volume is the default");
    assert_eq!(
        loud.properties(&props(), "Volume", 0.0)[VOLUME].default_value(),
        json!(0),
        "muted starts silent"
    );
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
