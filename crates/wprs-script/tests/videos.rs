mod common;

use common::{staged, text};
use serde_json::json;
use std::sync::Arc;
use wprs_scene::{Course, Runtime, Scene, Values, VideoCommand};
use wprs_script::Frame;

fn filmed(uses: &[&str]) -> serde_json::Value {
    json!({ "assets": { "rain": { "path": "rain.webm" } },
        "scripts": [{ "path": "main.js", "uses": uses }],
        "layers": [{ "id": "a", "type": "text", "text": "", "size": 10, "font": "serif" },
            { "id": "clip", "type": "video", "video": "rain", "playback": { "loop": false, "end": 3 } }] })
}

const SOURCE: &str = r#"import { scene } from "wprs";
    const clip = scene.layer("clip").video;
    let heard = 0;
    clip.on("ended", (e) => { heard += 1; });
    export function update(f) {
        if (f.time === 0) { clip.pause(); clip.rate = 2; }
        if (f.time === 1) { clip.seek(1); clip.seek(2.5); clip.play(); }
        if (f.time === 3) { clip.stop(); }
        scene.layer("a").text = `${clip.time} ${clip.duration} ${clip.playing} ${clip.rate} ${heard}`;
    }"#;

#[test]
fn scripts_drive_videos_and_hear_them_end() {
    let mut script = staged(filmed(&["animations", "layers"]), &[("main.js", SOURCE)]);
    let course =
        |time: f32, playing: bool| Course { id: "clip".into(), time, duration: 3.0, playing, speed: 1.0 };
    let out = script.update(&Frame { videos: &[course(0.0, true)], ..Frame::at(0.0) }).unwrap();
    assert_eq!(out.videos["clip"].commands, [VideoCommand::Pause, VideoCommand::Rate(2.0)]);
    assert_eq!(text(&out, "a"), "0 3 true 1 0");
    let out = script.update(&Frame { videos: &[course(0.5, false)], ..Frame::at(1.0) }).unwrap();
    let seeks = [VideoCommand::Seek(1.0), VideoCommand::Seek(2.5), VideoCommand::Play];
    assert_eq!(out.videos["clip"].commands, seeks);
    let ended = ["clip".to_string()];
    let out =
        script.update(&Frame { videos: &[course(3.0, false)], ended: &ended, ..Frame::at(2.0) }).unwrap();
    assert_eq!(text(&out, "a"), "3 3 false 1 1");
    let refused = staged(filmed(&["layers"]), &[("main.js", SOURCE)]);
    let message = |mut s: wprs_script::Script| {
        let out = s.update(&Frame::at(0.0)).unwrap();
        [out.console, out.stopped].concat().join(" ")
    };
    let said = message(refused);
    assert!(said.contains("uses animations without declaring it"), "{said}");
}

#[test]
fn the_runtime_plays_seeks_and_ends_videos_from_scripts() {
    let doc = filmed(&["animations"]);
    let mut text = doc.clone();
    (text["schema"], text["canvas"]) = (json!(wprs_scene::SCHEMA), json!({ "size": [1920, 1080] }));
    let scene = Scene::from_json(&text.to_string()).unwrap();
    let mut rt = Runtime::new(Arc::new(scene), Values::new());
    rt.set_lengths([("rain".to_string(), 3.0)].into());
    rt.advance(1.0);
    assert!((rt.tapes()[0].at - 1.0).abs() < 1e-6 && rt.tapes()[0].playing);
    let steer = |rt: &mut Runtime, commands: serde_json::Value| {
        let outputs =
            serde_json::from_value(json!({ "videos": { "clip": { "commands": commands } } })).unwrap();
        rt.set_script(outputs);
    };
    steer(&mut rt, json!([{ "seek": 0.5 }, { "seek": 2.0 }, { "rate": 2 }]));
    let jumps = rt.tapes()[0].jumps;
    assert_eq!((rt.tapes()[0].at, jumps), (2.0, 1), "only the last seek of a frame counts");
    rt.advance(0.4);
    assert!((rt.tapes()[0].at - 2.8).abs() < 1e-5);
    rt.advance(0.4);
    assert!(rt.tapes()[0].ended && rt.tapes()[0].at == 3.0);
    assert_eq!(rt.take_ended(), ["clip"]);
    let mut courses = Vec::new();
    rt.fill_videos(&mut courses);
    assert_eq!((courses[0].time, courses[0].duration, courses[0].playing), (3.0, 3.0, false));
    assert_eq!(rt.frame([0.0, 0.0, 1920.0, 1080.0], wprs_scene::Quality::Q3).wake, wprs_scene::Wake::Idle);
    steer(&mut rt, json!(["play"]));
    assert!(!rt.tapes()[0].ended && rt.tapes()[0].at == 0.0 && rt.tapes()[0].jumps == 2);
    steer(&mut rt, json!(["pause"]));
    rt.advance(1.0);
    assert_eq!(rt.tapes()[0].at, 0.0);
}
