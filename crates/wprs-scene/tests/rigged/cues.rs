use super::kit::{arm, fox};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_scene::{Runtime, SCHEMA, Scene, Values};

fn played(clip: Value, timelines: Value) -> Runtime {
    let layer = fox(json!({ "clips": [clip], "animations": [{ "clip": "c" }] }));
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [64, 64] }, "timelines": timelines,
        "assets": { "arm": { "path": "arm.glb" } }, "layers": [layer] });
    let mut scene = Scene::from_json(&text.to_string()).unwrap();
    let mut gl = arm();
    let times = gl.doc["animations"][0]["samplers"][0]["input"].as_u64().unwrap() as usize;
    gl.doc["accessors"][times]["max"] = json!([1.0]);
    let glb = gl.bytes();
    scene.adopt(&BTreeMap::new(), move |_: &str, _: usize| Ok::<_, String>(glb.clone())).unwrap();
    assert_eq!(scene.validate(&BTreeMap::new()), []);
    Runtime::new(Arc::new(scene), Values::new())
}

fn names(rt: &mut Runtime) -> Vec<String> {
    let fired = rt.take_fired().into_iter();
    fired.map(|f| format!("{}:{}", f.timeline, f.name.unwrap_or_default())).collect()
}

fn run(rt: &mut Runtime, seconds: f32) {
    (0..(seconds * 60.0).round() as usize).for_each(|_| rt.advance(1.0 / 60.0));
}

#[test]
fn clip_events_fire_once_per_pass_of_a_looping_clip() {
    let events = json!([{ "t": 0, "name": "zero" }, { "t": 0.5, "name": "half" }]);
    let mut rt = played(json!({ "id": "c", "animation": "wave", "events": events }), json!([]));
    rt.advance(0.05);
    assert_eq!(names(&mut rt), ["fox#c:zero"]);
    run(&mut rt, 0.5);
    assert_eq!(names(&mut rt), ["fox#c:half"]);
    run(&mut rt, 2.0);
    let looped = names(&mut rt);
    assert_eq!(looped.iter().filter(|n| *n == "fox#c:zero").count(), 2, "{looped:?}");
    assert_eq!(looped.iter().filter(|n| *n == "fox#c:half").count(), 2, "{looped:?}");
    rt.advance(1.0);
    assert_eq!(names(&mut rt), ["fox#c:zero", "fox#c:half"], "a long frame keeps the order of the clip");
}

#[test]
fn pingpong_clips_fire_their_events_each_way_and_once_clips_once() {
    let bounce = json!([{ "t": 0.25, "name": "quarter" }]);
    let mut rt =
        played(json!({ "id": "c", "animation": "wave", "loop": "pingpong", "events": bounce }), json!([]));
    run(&mut rt, 4.0);
    assert_eq!(names(&mut rt).len(), 4);
    let end = json!([{ "t": 1, "name": "end" }]);
    let mut rt = played(json!({ "id": "c", "animation": "wave", "loop": "once", "events": end }), json!([]));
    run(&mut rt, 3.0);
    assert_eq!(names(&mut rt), ["fox#c:end"]);
}

#[test]
fn a_clip_event_starts_the_timelines_it_triggers() {
    let blink = json!([{ "t": 0.5, "name": "blink" }]);
    let timelines = json!([{ "id": "eyes", "duration": 1, "trigger": "event:blink" }]);
    let mut rt = played(json!({ "id": "c", "animation": "wave", "events": blink }), timelines);
    run(&mut rt, 0.25);
    assert!(!rt.playbacks()[0].playing);
    run(&mut rt, 0.5);
    let eyes = &rt.playbacks()[0];
    assert!(eyes.playing && (eyes.time - 0.25).abs() < 0.02, "{eyes:?}");
}
