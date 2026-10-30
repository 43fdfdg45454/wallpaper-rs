mod common;

use common::{at, setup, staged, text};
use serde_json::json;
use wprs_scene::AudioFrame;
use wprs_script::{Frame, Limits, Script};

fn plain(files: &[(&str, &str)]) -> Script {
    let scripts: Vec<_> = files.iter().map(|(path, _)| json!({ "path": path })).collect();
    let layers = json!([{ "id": "a", "type": "text", "text": "", "size": 10, "font": "serif" }]);
    staged(json!({ "scripts": scripts, "layers": layers }), files)
}

#[test]
fn modules_that_declare_nothing_get_a_new_frame_with_plain_arrays_every_time() {
    let mut script = plain(&[(
        "main.js",
        r#"import { scene } from "wprs";
        let prev = null;
        export function update(f) {
            const d = prev ? f.time - prev.time : -1;
            prev = f;
            scene.layer("a").text = `${d} ${Array.isArray(f.audio.average)} ${JSON.stringify(f.audio.average.slice(0, 2))}`;
        }"#,
    )]);
    let mut audio = AudioFrame::default();
    audio.average[0] = 0.5;
    let texts: Vec<String> = [0.0, 0.5, 1.0]
        .map(|t| text(&script.update(&Frame { audio: Some(&audio), ..Frame::at(t) }).unwrap(), "a"))
        .into();
    assert_eq!(texts, ["-1 true [0.5,0]", "0.5 true [0.5,0]", "0.5 true [0.5,0]"]);
}

#[test]
fn modules_that_declare_nothing_keep_to_the_first_interface() {
    let main = r#"import { scene, engine, props, audio, input } from "wprs";
        let called = "";
        export function init() { called = "init"; }
        export function update() {
            const missing = [engine.device, engine.canvas, props.all, audio.buffers, input.cursor.left, scene.layer("a").skew];
            let refused = "";
            for (const f of [() => console.log("x"), () => setTimeout(() => {}, 10), () => (scene.layer("a").skew = [1, 1])]) {
                try { f(); } catch (e) { refused += `${e.name} `; }
            }
            scene.layer("a").text = `${called}|${missing.every((m) => m === undefined)}|${refused}`;
        }"#;
    let files = [
        ("main.js", main),
        ("shared.js", r#"import { shared } from "wprs"; export function update() {}"#),
        ("math.js", r#"import { Vec2 } from "wprs/math"; export function update() {}"#),
        ("lib.js", r#"import { x } from "./main.js"; export function update() {}"#),
    ];
    let mut script = plain(&files);
    let out = at(&mut script, 0.0).unwrap();
    assert_eq!(text(&out, "a"), "|true|TypeError TypeError TypeError ");
    assert_eq!(out.stopped.len(), 3, "{:?}", out.stopped);
    assert!(out.stopped.iter().all(|s| !s.starts_with("main.js")), "{:?}", out.stopped);
    assert!(out.stopped.iter().any(|s| s.starts_with("math.js") && s.contains("imports only wprs")));
}

#[test]
fn declared_modules_and_bound_modules_get_the_newer_interface() {
    let files = [
        (
            "main.js",
            r#"import { shared, console } from "wprs"; export function init() { shared.n = 2; console.log("hi"); }"#,
        ),
        ("bound.js", r#"import { shared } from "wprs"; export const twice = () => shared.n * 2;"#),
    ];
    let scene = json!({
        "scripts": [{ "path": "main.js", "events": ["init"] }, { "path": "bound.js" }],
        "layers": [{ "id": "a", "type": "text", "text": "", "size": 10, "font": "serif",
            "opacity": { "script": "bound.js", "fn": "twice" } }]
    });
    let made = setup(scene, &files);
    assert!(made.first.is_empty(), "{:?}", made.first);
    let mut script = Script::load(&made, Limits::default()).unwrap();
    let out = at(&mut script, 0.0).unwrap();
    assert!(out.stopped.is_empty() && out.console == ["log main.js: hi"], "{out:?}");
    assert_eq!(out.values.values().next(), Some(&json!(4)));
}

fn writes(declared: bool) -> Vec<(Option<f32>, Option<f32>, serde_json::Value)> {
    let main = r#"import { scene } from "wprs";
        const a = scene.layer("a");
        export function update({ time }) {
            if (time === 0) a.opacity = 0.25;
            a.rotation = (a.rotation ?? 0) + 1;
            return a.rotation;
        }"#;
    let mut entry = json!({ "path": "main.js" });
    if declared {
        entry["events"] = json!(["update"]);
    }
    let layers = json!([{ "id": "a", "type": "text", "text": "", "size": 10, "font": "serif" }]);
    let made = setup(json!({ "scripts": [entry], "layers": layers }), &[("main.js", main)]);
    assert_eq!(made.first.is_empty(), declared);
    let mut script = Script::load(&made, Limits::default()).unwrap();
    [0.0, 0.5, 1.0]
        .map(|t| {
            let out = at(&mut script, t).unwrap();
            let a = out.layers.get("a").cloned().unwrap_or_default();
            (a.opacity, a.rotation, out.values["main.js"].clone())
        })
        .into()
}

#[test]
fn modules_that_declare_nothing_write_layers_for_one_frame_only() {
    let once = [(Some(0.25), Some(1.0), json!(1)), (None, Some(1.0), json!(1)), (None, Some(1.0), json!(1))];
    assert_eq!(writes(false), once);
}

#[test]
fn declared_modules_keep_their_layer_writes() {
    let kept = [
        (Some(0.25), Some(1.0), json!(1)),
        (Some(0.25), Some(2.0), json!(2)),
        (Some(0.25), Some(3.0), json!(3)),
    ];
    assert_eq!(writes(true), kept);
}
