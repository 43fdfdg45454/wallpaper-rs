use super::*;

#[test]
fn an_own_clock_on_a_file_property_plays_the_default_video_until_a_file_is_picked() {
    let layers = json!([{ "id": "a", "type": "video", "video": { "prop": "pick" }, "size": [20, 20] },
        { "id": "b", "type": "video", "video": { "prop": "pick" }, "size": [20, 20], "playback": { "own_clock": true } }]);
    let doc = json!({ "schema": SCHEMA, "canvas": { "size": [100, 50] },
        "assets": { "clip": { "path": "clip.webm" } }, "layers": layers });
    let pick = json!({ "pick": { "type": "file", "accept": ["video"], "default_asset": "clip" } });
    let props: BTreeMap<String, Property> = serde_json::from_value(pick).unwrap();
    let scene = Scene::load(&doc.to_string(), &props).unwrap();
    assert!(scene.validate(&props).is_empty(), "{:?}", scene.validate(&props));
    assert_eq!(scene.lanes(), [("pick@b".to_string(), "pick".to_string())]);
    assert_eq!(scene.played_lanes(&props), [("pick@b".to_string(), "clip".to_string())]);
    let mut rt = Runtime::new(Arc::new(scene), effective(&props, &[]));
    assert_eq!(super::clocks::shown(&rt), ["clip", "pick@b"], "the own clock draws its lane at once");
    rt.set_props(serde_json::from_value(json!({ "pick": "clip" })).unwrap());
    assert_eq!(super::clocks::shown(&rt), ["clip", "pick@b"], "a reset keeps drawing the lane");
    let plain: BTreeMap<String, Property> =
        serde_json::from_value(json!({ "pick": { "type": "file", "accept": ["video"] } })).unwrap();
    let scene = Scene::load(&doc.to_string(), &plain).unwrap();
    assert_eq!(scene.played_lanes(&plain), [("pick@b".to_string(), "pick".to_string())]);
}
