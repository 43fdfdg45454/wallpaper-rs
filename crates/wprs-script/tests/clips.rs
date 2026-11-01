mod common;

use common::{staged, text};
use serde_json::json;
use wprs_scene::{Course, VideoCommand};
use wprs_script::Frame;

fn puppet(uses: &[&str]) -> serde_json::Value {
    json!({ "assets": { "rig": { "path": "fox.glb" } },
        "scripts": [{ "path": "main.js", "uses": uses }],
        "layers": [{ "id": "a", "type": "text", "text": "", "size": 10, "font": "serif" },
            { "id": "fox", "type": "mesh", "mesh": "rig", "clips": [{ "id": "wave" }] }] })
}

const SOURCE: &str = r#"import { scene } from "wprs";
    const wave = scene.layer("fox").anim("wave");
    let heard = "";
    wave.on("ended", (e) => { heard = `${e.layer}/${e.clip}`; });
    export function update(f) {
        if (f.time === 0) { wave.play(); wave.weight = 0.5; wave.rate = 2; }
        if (f.time === 1) { wave.seek(0.25); wave.pause(); wave.weight = 7; }
        scene.layer("a").text = `${wave.time} ${wave.duration} ${wave.playing} ${wave.rate} ${wave.weight} ${heard}`;
    }"#;

#[test]
fn scripts_play_weigh_seek_and_hear_mesh_clips() {
    let mut script = staged(puppet(&["animations", "layers"]), &[("main.js", SOURCE)]);
    let course =
        |time: f32, playing: bool| Course { id: "fox#wave".into(), time, duration: 1.0, playing, speed: 2.0 };
    let out = script.update(&Frame { videos: &[course(0.0, false)], ..Frame::at(0.0) }).unwrap();
    assert_eq!(
        out.videos["fox#wave"].commands,
        [VideoCommand::Play, VideoCommand::Weight(0.5), VideoCommand::Rate(2.0)]
    );
    assert_eq!(text(&out, "a"), "0 1 false 2 0.5 ");
    let ended = ["fox#wave".to_string()];
    let out =
        script.update(&Frame { videos: &[course(1.0, true)], ended: &ended, ..Frame::at(1.0) }).unwrap();
    let wanted = [VideoCommand::Seek(0.25), VideoCommand::Pause, VideoCommand::Weight(1.0)];
    assert_eq!(out.videos["fox#wave"].commands, wanted);
    assert_eq!(text(&out, "a"), "1 1 true 2 1 fox/wave");
}

#[test]
fn clips_need_the_animations_part() {
    let mut refused = staged(puppet(&["layers"]), &[("main.js", SOURCE)]);
    let out = refused.update(&Frame::at(0.0)).unwrap();
    let said = [out.console, out.stopped].concat().join(" ");
    assert!(said.contains("uses animations without declaring it"), "{said}");
    assert!(out.videos.is_empty());
}
