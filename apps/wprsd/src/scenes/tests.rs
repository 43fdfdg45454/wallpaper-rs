use super::lanes;
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
    lanes(&scene, &mut videos);
    assert_eq!(videos.len(), 2);
    let (key, reel) = &videos[1];
    assert_eq!((key.as_str(), &reel.path, reel.lane.as_str()), ("clip@b", &videos[0].1.path, "clip@b"));
    assert_ne!(*reel, videos[0].1, "the decoder of the own clock is not shared");
}
