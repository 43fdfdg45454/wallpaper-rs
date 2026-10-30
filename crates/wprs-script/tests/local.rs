mod common;

use common::{setup, staged, text};
use serde_json::json;
use std::time::{Duration, Instant};
use wprs_scene::{ScriptOutputs, SoundCommand, Touch, TouchKind};
use wprs_script::{Error, Frame, Limits, Local};

fn sounding() -> serde_json::Value {
    json!({ "assets": { "bell": { "path": "bell.ogg" } },
        "scripts": [{ "path": "main.js", "uses": ["sounds", "layers"] }],
        "layers": [{ "id": "a", "type": "text", "text": "", "size": 10, "font": "serif" },
            { "id": "bell", "type": "sound", "sound": "bell" }] })
}

fn answer(local: &mut Local, frame: &Frame) -> Result<ScriptOutputs, Error> {
    let end = Instant::now() + Duration::from_secs(5);
    while Instant::now() < end {
        if let Some(out) = local.update(frame)? {
            return Ok(out);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("no answer from the script thread");
}

#[test]
fn scripts_play_pause_and_stop_sounds_and_set_their_volume() {
    let source = r#"import { sound, scene } from "wprs";
        export function update(f) {
            const bell = sound("bell");
            if (f.time === 0) { bell.play(); bell.volume = 2; }
            if (f.time === 1) { bell.pause(); bell.volume = "x"; }
            if (f.time === 2) { bell.stop(); bell.play(); }
            scene.layer("a").text = `${bell.playing} ${bell.volume}`;
        }"#;
    let mut script = staged(sounding(), &[("main.js", source)]);
    let sent = |script: &mut wprs_script::Script, time: f64, sounds: &[String]| {
        script.update(&Frame { sounds, ..Frame::at(time) }).unwrap()
    };
    let out = sent(&mut script, 0.0, &[]);
    assert_eq!(
        (text(&out, "a"), out.sounds["bell"].commands.clone()),
        ("true 1".into(), vec![SoundCommand::Play])
    );
    assert_eq!(out.sounds["bell"].volume, Some(1.0));
    let out = sent(&mut script, 0.5, &["bell".into()]);
    assert_eq!((text(&out, "a"), out.sounds["bell"].commands.len()), ("true 1".into(), 0));
    assert!(script.update(&Frame::at(1.0)).is_ok());
    let out = sent(&mut script, 2.0, &[]);
    assert_eq!(out.sounds["bell"].commands, [SoundCommand::Stop, SoundCommand::Play]);
    let refused = staged(
        json!({ "scripts": [{ "path": "main.js", "uses": ["layers"] }], "layers": [{ "id": "a", "type": "text", "text": "", "size": 10, "font": "serif" }] }),
        &[(
            "main.js",
            r#"import { sound, scene } from "wprs";
            let message = ""; try { sound("bell"); } catch (e) { message = e.message; }
            export function update() { scene.layer("a").text = message; }"#,
        )],
    );
    let out = {
        let mut refused = refused;
        refused.update(&Frame::at(0.0)).unwrap()
    };
    assert_eq!(text(&out, "a"), "main.js uses sounds without declaring it in scene.json");
}

#[test]
fn every_pointer_event_reaches_its_layer_handlers() {
    let source = r#"import { scene } from "wprs";
        const seen = [];
        for (const e of ["click", "enter", "leave", "move", "down", "up"]) scene.layer("b").on(e, (t) => seen.push(`${t.type}@${t.local}`));
        export function update() { scene.layer("a").text = seen.join(" "); }"#;
    let doc = json!({ "scripts": [{ "path": "main.js", "events": ["click", "enter", "leave", "move", "down", "up"], "uses": ["layers"] }],
        "layers": [{ "id": "a", "type": "text", "text": "", "size": 10, "font": "serif" },
            { "id": "b", "type": "solid", "color": "#ffffff", "size": [4, 4], "solid": true }] });
    let mut script = staged(doc, &[("main.js", source)]);
    let kinds = [
        TouchKind::Enter,
        TouchKind::Move,
        TouchKind::Down,
        TouchKind::Up,
        TouchKind::Click,
        TouchKind::Leave,
    ];
    let touches: Vec<Touch> = kinds
        .iter()
        .enumerate()
        .map(|(i, kind)| Touch { layer: "b".into(), kind: *kind, local: [i as f32, 0.0], scene: [0.0; 2] })
        .collect();
    let out = script.update(&Frame { touches: &touches, ..Frame::at(0.0) }).unwrap();
    assert_eq!(text(&out, "a"), "enter@0,0 move@1,0 down@2,0 up@3,0 click@4,0 leave@5,0");
}

#[test]
fn a_script_thread_answers_frames_and_turns_off_when_it_hangs() {
    let fine = setup(
        json!({ "layers": [{ "id": "a", "type": "text", "text": "", "size": 10, "font": "serif" }] }),
        &[(
            "main.js",
            r#"import { scene } from "wprs"; export function update(f) { scene.layer("a").text = `t ${f.time}`; }"#,
        )],
    );
    let mut local = Local::new(&fine, Limits::default());
    let out = answer(&mut local, &Frame::at(2.0)).unwrap();
    assert!(text(&out, "a").starts_with("t "), "{out:?}");
    assert!(local.progress().0 >= 1);
    let hung = setup(json!({}), &[("main.js", "export function update() { for (;;) {} }")]);
    let mut local = Local::new(&hung, Limits { update: Duration::from_millis(2), ..Limits::default() });
    let end = Instant::now() + Duration::from_secs(10);
    let mut last = Ok(None);
    while Instant::now() < end && last != Err(Error::Disabled) {
        last = local.update(&Frame::at(0.0));
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(last, Err(Error::Disabled), "a hung script thread kept the scene's scripts on");
    let broken = setup(json!({}), &[("main.js", "export function update( {")]);
    let mut local = Local::new(&broken, Limits::default());
    assert!(answer(&mut local, &Frame::at(0.0)).is_ok_and(|out| !out.stopped.is_empty()));
}

#[test]
fn a_slow_load_within_its_budget_keeps_the_scripts_on() {
    let slow = setup(
        json!({ "layers": [{ "id": "a", "type": "text", "text": "", "size": 10, "font": "serif" }] }),
        &[(
            "main.js",
            r#"import { scene } from "wprs";
            const start = Date.now();
            while (Date.now() - start < 300) {}
            export function update(f) { scene.layer("a").text = `t ${f.time}`; }"#,
        )],
    );
    let mut local = Local::new(&slow, Limits { load: Duration::from_millis(400), ..Limits::default() });
    let out = answer(&mut local, &Frame::at(1.0)).expect("a load slower than 250 ms turned the scripts off");
    assert_eq!(text(&out, "a"), "t 1");
}
