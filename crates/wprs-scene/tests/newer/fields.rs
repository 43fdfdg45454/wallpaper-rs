use super::*;

#[test]
fn every_new_field_parses_validates_and_names_its_feature() {
    let scene = full();
    assert_eq!(scene.features().into_iter().collect::<Vec<_>>(), EXPECTED);
    let Kind::Text(title) = &scene.layer("title").unwrap().kind else { panic!("title is no text") };
    assert!(title.laid_out() && title.styled() && title.fallback == ["sans", "serif"]);
    let Kind::Light(lamp) = &scene.layer("lamp").unwrap().kind else { panic!("lamp is no light") };
    assert_eq!(
        (lamp.light, lamp.cone, lamp.volumetric.map(|v| v.samples)),
        (Beam::Spot, Some([20.0, 40.0]), Some(16))
    );
    let Kind::Sound(music) = &scene.layer("music").unwrap().kind else { panic!("music is no sound") };
    assert_eq!(music.clips().collect::<Vec<_>>(), ["theme", "wind"]);
    assert_eq!((music.mode, music.pause, music.stream), (Some(Order::Random), Some([2.0, 8.0]), true));
    assert_eq!(scene.layer("sky").unwrap().blend, Blend::LinearBurn);
    assert!(Blend::LinearBurn.newer() && !Blend::Luminosity.newer() && BLENDS.len() == 34);
    assert_eq!(scene.layer("sky").unwrap().parallax, Parallax([0.1, 0.02]));
    assert_eq!(scene.assets["run"].cells(), 8);
    assert!(
        scene.assets["cat"].moves() && scene.assets["sky"].sampled() && scene.assets["serif"].collection()
    );
    assert_eq!(scene.timelines[0].events, [Event { t: 2.0, name: "drop".into() }]);
    assert!(scene.paths().contains("effects/ripple.wgsl") && scene.paths().contains("effects/ripple.json"));
    assert!(needs(&scene).cursor && needs(&scene).audio);
}

#[test]
fn new_fields_round_trip_and_older_scenes_use_none_of_them() {
    let scene = full();
    let text = scene.to_json().unwrap();
    let again = Scene::from_json(&text).unwrap();
    assert_eq!(again, scene);
    assert_eq!(again.features(), scene.features());
    let written: Json = serde_json::from_str(&text).unwrap();
    assert_eq!(written["layers"][0]["parallax"], json!([0.1, 0.02]));
    assert_eq!(written["layers"][9]["opacity"]["keys"][0][2], json!({ "out": [0.5, 1.0] }));
    assert!(written["canvas"].get("clear").is_some() && written["layers"][7].get("redraw").is_some());
    let old = Scene::from_json(SAMPLE).unwrap();
    assert_eq!(old.features(), Default::default());
    let plain: Json = serde_json::from_str(&old.to_json().unwrap()).unwrap();
    assert!(plain["canvas"].get("clear").is_none() && plain.get("effects").is_none());
    let even = json!({ "schema": SCHEMA, "canvas": { "size": [8, 8] },
        "layers": [{ "id": "a", "type": "solid", "color": "#000000", "parallax": 0.4 }] });
    let even = Scene::from_json(&even.to_string()).unwrap();
    assert!(even.features().is_empty() && even.to_json().unwrap().contains("\"parallax\": 0.4"));
}

#[test]
fn clock_texts_keep_their_letters_and_date_texts_name_their_feature() {
    let text = |t: Json| {
        json!({ "schema": SCHEMA, "canvas": { "size": [100, 50] }, "layers": [{ "id": "t", "type": "text", "text": t }] })
            .to_string()
    };
    let scene = Scene::load(&text(json!({ "clock": "HH:mm Sydney" })), &BTreeMap::new()).unwrap();
    assert!(scene.features().is_empty());
    let mut rt = Runtime::new(std::sync::Arc::new(scene), Values::new());
    rt.set_wall(13.0 * 3600.0 + 5.0 * 60.0);
    let frame = rt.frame([0.0, 0.0, 100.0, 50.0], Quality::Q3);
    assert!(matches!(&frame.draws[0].content, Content::Text { text, .. } if text == "13:05 Sydney"));
    let dated = Scene::load(&text(json!({ "date": "d MMMM yyyy" })), &BTreeMap::new()).unwrap();
    assert!(dated.features().contains("clock_dates"));
    let wrong = json!({ "schema": SCHEMA, "canvas": { "size": [100, 50] },
        "layers": [{ "id": "s", "type": "solid", "color": "#000000", "opacity": { "date": "d" } }] });
    assert!(Scene::load(&wrong.to_string(), &BTreeMap::new()).is_err());
}

#[test]
fn video_lengths_come_from_their_headers() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    for name in ["magenta-64x32.webm", "quadrants-64x32.webm"] {
        let bytes = std::fs::read(dir.join(name)).unwrap();
        let seconds = wprs_scene::film_seconds(&bytes, &[]).unwrap();
        assert!(seconds > 0.0 && seconds < 60.0, "{name}: {seconds}");
    }
    let mut mp4 = vec![0, 0, 0, 108];
    mp4.extend(b"mvhd");
    mp4.extend([0; 12]);
    mp4.extend(1000u32.to_be_bytes());
    mp4.extend(2500u32.to_be_bytes());
    assert_eq!(wprs_scene::film_seconds(b"junk", &mp4), Some(2.5));
    assert_eq!(wprs_scene::film_seconds(b"junk", b"more junk"), None);
}
