use super::*;

fn filmed(layers: serde_json::Value) -> Runtime {
    runtime(
        &json!({ "schema": SCHEMA, "canvas": { "size": [100, 50] }, "layers": layers,
            "assets": { "clip": { "path": "clip.webm" } } })
        .to_string(),
    )
}

fn video(id: &str, playback: serde_json::Value) -> serde_json::Value {
    json!({ "id": id, "type": "video", "video": "clip", "size": [20, 20], "playback": playback })
}

fn shown(rt: &Runtime) -> Vec<String> {
    let frame = rt.frame(VIEW, Quality::Q3);
    let assets = frame.draws.iter().filter_map(|d| match &d.content {
        Content::Video { asset, .. } => Some(asset.to_string()),
        _ => None,
    });
    assets.collect()
}

#[test]
fn layers_that_share_a_video_follow_one_clock_unless_one_asks_for_its_own() {
    let shared = filmed(json!([video("a", json!({})), video("b", json!({ "rate": 2 }))]));
    assert_eq!(shared.tapes().len(), 1);
    assert_eq!(shown(&shared), ["clip", "clip"]);
    let layers = json!([
        video("a", json!({})),
        video("b", json!({ "own_clock": true, "start": 1, "rate": 2 })),
        video("c", json!({}))
    ]);
    let mut own = filmed(layers);
    let tapes: Vec<(String, Option<String>, f64)> =
        own.tapes().iter().map(|t| (t.asset.clone(), t.layer.clone(), t.start)).collect();
    assert_eq!(tapes, [("clip".into(), Some("a".into()), 0.0), ("clip@b".into(), Some("b".into()), 1.0)]);
    assert_eq!(shown(&own), ["clip", "clip@b", "clip"]);
    own.frame(VIEW, Quality::Q3);
    own.advance(0.5);
    let at: Vec<f64> = own.tapes().iter().map(|t| t.at).collect();
    assert!((at[0] - 0.5).abs() < 0.05 && (at[1] - 2.0).abs() < 0.05, "{at:?}");
    let seek = VideoControl { commands: vec![VideoCommand::Seek(3.0)] };
    own.set_script(ScriptOutputs { videos: [("b".to_string(), seek)].into(), ..ScriptOutputs::default() });
    let at: Vec<f64> = own.tapes().iter().map(|t| t.at).collect();
    assert!((at[0] - 0.5).abs() < 0.05 && (at[1] - 3.0).abs() < 0.01, "{at:?}");
    assert_eq!(own.scene.lanes(), [("clip@b".to_string(), "clip".to_string())]);
}

#[test]
fn own_clocks_are_a_feature_and_take_only_a_flag() {
    let doc = |clock: serde_json::Value| {
        json!({ "schema": SCHEMA, "canvas": { "size": [100, 50] }, "assets": { "clip": { "path": "clip.webm" } },
            "layers": [video("a", json!({ "own_clock": clock }))] })
        .to_string()
    };
    let scene = Scene::from_json(&doc(json!(true))).unwrap();
    assert!(scene.features().contains("video_clocks"), "{:?}", scene.features());
    let shared = Scene::from_json(&doc(json!(false))).unwrap();
    assert!(!shared.features().contains("video_clocks"));
    assert!(Scene::from_json(&doc(json!("own"))).is_err());
}

#[test]
fn own_clocks_are_refused_where_no_named_video_file_plays() {
    let doc = |layer: serde_json::Value| {
        json!({ "schema": SCHEMA, "canvas": { "size": [100, 50] },
            "assets": { "clip": { "path": "clip.webm" }, "sky": { "path": "sky.png" } },
            "pools": { "bars": { "type": "mesh", "max": 2, "max_vertices": 8 } }, "layers": [layer] })
        .to_string()
    };
    let clock = json!({ "own_clock": true });
    let refused = [
        json!({ "id": "a", "type": "image", "image": "sky", "playback": clock }),
        json!({ "id": "a", "type": "video", "video": "bars", "playback": clock }),
        json!({ "id": "a", "type": "video", "video": { "prop": "film" }, "playback": clock }),
        json!({ "id": "a", "type": "video", "video": { "prop": "film", "map": { "x": "bars" } }, "playback": clock }),
    ];
    for layer in refused {
        let issues = Scene::from_json(&doc(layer.clone())).unwrap().validate(&Default::default());
        let said: Vec<String> = issues.iter().map(ToString::to_string).collect();
        assert!(said.iter().any(|i| i.contains("/layers/0/playback/own_clock")), "{layer}: {said:?}");
    }
    let shared = json!({ "id": "a", "type": "video", "video": "bars", "playback": { "own_clock": false } });
    assert!(!Scene::load(&doc(shared), &Default::default()).unwrap().features().contains("video_clocks"));
}

#[test]
fn a_video_picked_by_a_property_keeps_a_clock_of_its_own_for_each_video_it_may_show() {
    let pick = json!({ "prop": "style", "map": { "night": "clip", "dawn": "dusk" } });
    let doc = json!({ "schema": SCHEMA, "canvas": { "size": [100, 50] },
        "assets": { "clip": { "path": "clip.webm" }, "dusk": { "path": "dusk.webm" } },
        "layers": [video("shared", json!({})),
            { "id": "own", "type": "video", "video": pick, "size": [20, 20], "playback": { "own_clock": true, "rate": 2 } }] });
    let mut rt = runtime(&doc.to_string());
    assert_eq!(rt.scene.lanes(), [("clip@own".into(), "clip".into()), ("dusk@own".into(), "dusk".into())]);
    assert!(rt.scene.validate(&props()).is_empty(), "{:?}", rt.scene.validate(&props()));
    let keys: Vec<String> = rt.tapes().iter().map(|t| t.asset.clone()).collect();
    assert_eq!(keys, ["clip", "dusk", "clip@own", "dusk@own"]);
    assert_eq!(shown(&rt), ["clip", "clip@own"]);
    rt.frame(VIEW, Quality::Q3);
    rt.advance(0.5);
    let at = |rt: &Runtime, key: &str| rt.tapes().iter().find(|t| t.asset == key).unwrap().at;
    assert!((at(&rt, "clip@own") - 1.0).abs() < 0.05 && at(&rt, "dusk@own") == 0.0);
    let seek = VideoControl { commands: vec![VideoCommand::Seek(3.0)] };
    rt.set_script(ScriptOutputs { videos: [("own".to_string(), seek)].into(), ..ScriptOutputs::default() });
    assert!((at(&rt, "clip@own") - 3.0).abs() < 0.01 && (at(&rt, "dusk@own") - 3.0).abs() < 0.01);
    let mut courses = Vec::new();
    rt.fill_videos(&mut courses);
    assert_eq!(courses.iter().filter(|c| c.id == "own").count(), 1);
}

#[test]
fn a_folder_video_a_script_shows_runs_on_a_clock_scripts_steer() {
    let mut rt = filmed(json!([video("a", json!({ "rate": 2 }))]));
    let file = "wpak-user://album/abc".to_string();
    let set = LayerOverride { file: Some(file.clone()), ..Default::default() };
    let shows = |videos| ScriptOutputs {
        layers: [("a".to_string(), set.clone())].into(),
        videos,
        ..Default::default()
    };
    rt.set_script(shows(Default::default()));
    let tape = rt.tapes().iter().find(|t| t.asset == file).cloned().expect("a clock for the shown file");
    assert_eq!((tape.layer.as_deref(), tape.repeat), (Some("a"), true));
    let seek = VideoControl {
        commands: vec![VideoCommand::Seek(3.0), VideoCommand::Pause, VideoCommand::Volume(2.0)],
    };
    rt.set_script(shows([("a".to_string(), seek)].into()));
    let tape = rt.tapes().iter().find(|t| t.asset == file).unwrap();
    assert!((tape.at - 3.0).abs() < 0.01 && tape.volume == 1.0, "{tape:?}");
    let quiet = VideoControl { commands: vec![VideoCommand::Volume(0.25), VideoCommand::Volume(f32::NAN)] };
    rt.set_script(shows([("a".to_string(), quiet)].into()));
    assert_eq!(
        rt.tapes().iter().find(|t| t.asset == file).unwrap().volume,
        0.25,
        "a script turns the video down"
    );
    rt.set_script(ScriptOutputs::default());
    assert!(!rt.tapes().iter().any(|t| t.asset == file), "the clock goes with the file");
}

#[test]
fn a_folder_video_that_does_not_loop_ends_once_its_length_is_known() {
    let mut rt = filmed(json!([video("a", json!({ "loop": false }))]));
    let file = "wpak-user://album/abc".to_string();
    let set = LayerOverride { file: Some(file.clone()), ..Default::default() };
    rt.set_script(ScriptOutputs { layers: [("a".to_string(), set)].into(), ..Default::default() });
    assert!(rt.plays_once("a") && !rt.plays_once("missing"));
    rt.set_length(&file, 1.0);
    rt.set_length("nothing", f32::NAN);
    rt.frame(VIEW, Quality::Q3);
    rt.advance(1.5);
    rt.frame(VIEW, Quality::Q3);
    assert_eq!(rt.take_ended(), ["a"], "the layer tells scripts its video ended");
    let mut courses = Vec::new();
    rt.fill_videos(&mut courses);
    assert!(courses.iter().any(|c| c.id == "a" && c.duration == 1.0 && !c.playing), "{courses:?}");
}

#[test]
fn a_video_a_file_property_picks_keeps_a_clock_of_its_own() {
    let layers = json!([{ "id": "a", "type": "video", "video": { "prop": "pick" }, "size": [20, 20] },
        { "id": "b", "type": "video", "video": { "prop": "pick" }, "size": [20, 20], "playback": { "own_clock": true, "rate": 2 } }]);
    let doc = json!({ "schema": SCHEMA, "canvas": { "size": [100, 50] }, "layers": layers });
    let props: BTreeMap<String, Property> =
        serde_json::from_value(json!({ "pick": { "type": "file", "accept": ["video"] } })).unwrap();
    let scene = Scene::load(&doc.to_string(), &props).unwrap();
    assert!(scene.validate(&props).is_empty(), "{:?}", scene.validate(&props));
    let mut rt = Runtime::new(Arc::new(scene), effective(&props, &[]));
    assert_eq!(rt.scene.lanes(), [("pick@b".to_string(), "pick".to_string())]);
    rt.set_props(serde_json::from_value(json!({ "pick": "/videos/sea.webm" })).unwrap());
    assert_eq!(shown(&rt), ["/videos/sea.webm", "pick@b"], "the second layer plays its own clock");
    rt.frame(VIEW, Quality::Q3);
    rt.advance(0.5);
    let at = |rt: &Runtime, key: &str| rt.tapes().iter().find(|t| t.asset == key).map(|t| t.at);
    assert!(at(&rt, "pick@b").is_some_and(|t| (t - 1.0).abs() < 0.05), "{:?}", rt.tapes());
}
