use super::kit::runtime;
use serde_json::json;
use wprs_scene::{Quality, Runtime, Wake};

const VIEW: [f32; 4] = [0.0, 0.0, 64.0, 64.0];

fn wake(rt: &Runtime, q: Quality) -> Wake {
    rt.frame(VIEW, q).wake
}

fn generation(rt: &Runtime) -> u64 {
    rt.puppet("fox").unwrap().posed().generation
}

fn waving(repeat: &str) -> Runtime {
    runtime(
        json!({ "clips": [{ "id": "wave", "animation": "wave", "loop": repeat }], "animations": [{ "clip": "wave" }] }),
    )
}

#[test]
fn a_still_skeleton_asks_for_no_frames() {
    let mut rt = runtime(json!({ "shapes": { "lift": 0.5 } }));
    assert_eq!(wake(&rt, Quality::Q3), Wake::Idle);
    let first = generation(&rt);
    rt.advance(1.0);
    assert_eq!((wake(&rt, Quality::Q3), generation(&rt)), (Wake::Idle, first));
}

#[test]
fn a_clip_played_once_settles_and_tells_scripts_it_ended() {
    let mut rt = waving("once");
    rt.advance(0.5);
    assert_eq!(wake(&rt, Quality::Q3), Wake::Frame);
    rt.advance(0.6);
    assert_eq!(rt.take_ended(), ["fox#wave"]);
    assert_eq!(wake(&rt, Quality::Q3), Wake::Idle);
    let settled = generation(&rt);
    rt.advance(2.0);
    assert_eq!((generation(&rt), wake(&rt, Quality::Q3)), (settled, Wake::Idle));
    assert!(rt.take_ended().is_empty());
}

#[test]
fn a_repeating_clip_keeps_moving_and_paces_at_the_lowest_quality() {
    let mut rt = waving("repeat");
    rt.advance(0.25);
    assert_eq!(wake(&rt, Quality::Q3), Wake::Frame);
    assert_eq!(rt.pace(), u32::MAX);
    assert_eq!(wake(&rt, Quality::Q0), Wake::Frame);
    assert_eq!(rt.pace(), 30);
}

#[test]
fn scripts_pause_seek_and_read_clips() {
    let mut rt = waving("pingpong");
    let steer = |rt: &mut Runtime, commands: serde_json::Value| {
        rt.set_script(
            serde_json::from_value(json!({ "videos": { "fox#wave": { "commands": commands } } })).unwrap(),
        );
    };
    steer(&mut rt, json!([{ "seek": 0.75 }, "pause", { "rate": 2 }]));
    assert_eq!(wake(&rt, Quality::Q3), Wake::Frame, "a command wakes the scene once");
    rt.advance(0.5);
    let mut courses = Vec::new();
    rt.fill_videos(&mut courses);
    let c = courses.iter().find(|c| c.id == "fox#wave").unwrap();
    assert_eq!((c.time, c.duration, c.playing, c.speed), (0.75, 1.0, false, 2.0));
    assert_eq!(wake(&rt, Quality::Q3), Wake::Idle);
    steer(&mut rt, json!(["play", { "weight": 0 }]));
    rt.advance(0.1);
    let m = rt.puppet("fox").unwrap().posed().palette[1];
    assert!((m[0] - 1.0).abs() < 1e-5, "a layer of weight 0 leaves the rest pose");
    steer(&mut rt, json!([{ "weight": 1 }]));
    rt.advance(0.1);
    assert!(rt.puppet("fox").unwrap().posed().palette[1][0] < 0.99);
}

#[test]
fn scripts_add_layers_for_clips_that_have_none() {
    let mut rt = runtime(json!({ "clips": [{ "id": "wave", "animation": "wave", "loop": "once" }] }));
    assert!(rt.puppet("fox").unwrap().posed().palette[1][0] > 0.9999);
    let play = json!({ "videos": { "fox#wave": { "commands": ["play"] } } });
    rt.set_script(serde_json::from_value(play).unwrap());
    rt.advance(1.5);
    assert!(rt.puppet("fox").unwrap().posed().palette[1][0].abs() < 1e-4, "the arm turned by 90 degrees");
    assert_eq!(rt.take_ended(), ["fox#wave"]);
}

#[test]
fn a_script_plays_a_disabled_layer_and_stops_it_again() {
    let mut rt = runtime(json!({ "clips": [{ "id": "bend", "bones": { "arm": { "rotation": [[0, 90]] } } }],
        "animations": [{ "clip": "bend", "enabled": false }] }));
    let arm = |rt: &Runtime| rt.puppet("fox").unwrap().posed().palette[1][0];
    assert!(arm(&rt) > 0.9999);
    let steer = |rt: &mut Runtime, command: &str| {
        rt.set_script(
            serde_json::from_value(json!({ "videos": { "fox#bend": { "commands": [command] } } })).unwrap(),
        );
        rt.advance(0.1);
    };
    steer(&mut rt, "play");
    assert!(arm(&rt).abs() < 1e-4);
    steer(&mut rt, "stop");
    assert!(arm(&rt) > 0.9999);
}
