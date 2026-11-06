use super::{lanes, picked};
use crate::compositor::Reel;
use serde_json::json;
use wprs_scene::{SCHEMA, Scene};

#[test]
fn layers_with_their_own_clock_get_a_reel_of_their_own() {
    let layers = json!([{ "id": "a", "type": "video", "video": "clip" },
        { "id": "b", "type": "video", "video": "clip", "playback": { "own_clock": true } },
        { "id": "c", "type": "video", "video": "clip", "playback": { "own_clock": false } }]);
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [64, 32] }, "assets": { "clip": { "path": "clip.webm" } },
        "layers": layers });
    let scene = Scene::from_json(&text.to_string()).unwrap();
    let mut videos = vec![("clip".to_string(), Reel::plain("/tmp/clip.webm".into()))];
    lanes(&scene, &Default::default(), &mut videos);
    assert_eq!(videos.len(), 2);
    let (key, reel) = &videos[1];
    assert_eq!((key.as_str(), &reel.path, reel.lane.as_str()), ("clip@b", &videos[0].1.path, "clip@b"));
    assert_ne!(*reel, videos[0].1, "the decoder of the own clock is not shared");
}

#[test]
fn a_video_a_file_property_picks_gets_a_reel_for_each_own_clock() {
    let layers = json!([{ "id": "a", "type": "video", "video": { "prop": "pick" } },
        { "id": "b", "type": "video", "video": { "prop": "pick" }, "playback": { "own_clock": true } }]);
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [64, 32] }, "layers": layers });
    let scene = Scene::from_json(&text.to_string()).unwrap();
    let reels = picked(&scene, "pick", std::path::Path::new("/tmp/chosen.webm"));
    assert_eq!(reels.len(), 1);
    assert_eq!((reels[0].0.as_str(), reels[0].1.lane.as_str()), ("pick@b", "pick@b"));
    assert!(picked(&scene, "other", std::path::Path::new("/tmp/chosen.webm")).is_empty());
}

#[test]
fn an_own_clock_on_a_file_property_plays_its_default_video_until_a_file_is_picked() {
    let layers = json!([{ "id": "a", "type": "video", "video": { "prop": "pick" } },
        { "id": "b", "type": "video", "video": { "prop": "pick" }, "playback": { "own_clock": true } }]);
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [64, 32] }, "assets": { "clip": { "path": "clip.webm" } },
        "layers": layers });
    let scene = Scene::from_json(&text.to_string()).unwrap();
    let pick =
        json!({ "pick": { "type": "file", "accept": ["video"], "default_asset": "clip", "label": "Pick" } });
    let properties = serde_json::from_value(pick).unwrap();
    let mut videos = vec![("clip".to_string(), Reel::plain("/tmp/clip.webm".into()))];
    lanes(&scene, &properties, &mut videos);
    let own = videos.iter().find(|(key, _)| key == "pick@b").map(|(_, reel)| reel);
    assert_eq!(own.map(|r| (r.path.clone(), r.lane.as_str())), Some((videos[0].1.path.clone(), "pick@b")));
    let reels = picked(&scene, "pick", std::path::Path::new("/tmp/chosen.webm"));
    assert_eq!(reels.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(), ["pick@b"]);
}
