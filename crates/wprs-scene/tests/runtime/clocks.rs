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
        json!({ "id": "a", "type": "video", "video": { "prop": "film", "map": { "x": "clip" } }, "playback": clock }),
    ];
    for layer in refused {
        let issues = Scene::from_json(&doc(layer.clone())).unwrap().validate(&Default::default());
        let said: Vec<String> = issues.iter().map(ToString::to_string).collect();
        assert!(said.iter().any(|i| i.contains("/layers/0/playback/own_clock")), "{layer}: {said:?}");
    }
    let shared = json!({ "id": "a", "type": "video", "video": "bars", "playback": { "own_clock": false } });
    assert!(!Scene::load(&doc(shared), &Default::default()).unwrap().features().contains("video_clocks"));
}
