use serde_json::{Value, json};
use std::time::{Duration, Instant};
use wprs_scene::{AudioFrame, LayerOverride, PointerState, ScriptOutputs, TimelineCommand};
use wprs_script::{Error, Frame, Limits, Script};

fn update(script: &mut Script, time: f64) -> Result<ScriptOutputs, Error> {
    let props = [("glow".to_string(), json!(0.5))].into();
    let audio = AudioFrame { bass: 0.25, beat: true, ..AudioFrame::default() };
    let pointer =
        PointerState { pos: Some([0.25, 0.75]), buttons: 1, inside: true, ..PointerState::default() };
    script.update(&Frame { time, dt: 0.016, props: &props, audio: &audio, pointer: &pointer, actions: &[] })
}

fn sources(source: &str) -> Vec<(String, String)> {
    vec![("main.js".into(), source.into())]
}

fn load(source: &str) -> Script {
    Script::load(&sources(source), 7, ("en", &json!({})), Limits::default()).unwrap()
}

fn layer(outputs: &ScriptOutputs, id: &str) -> LayerOverride {
    outputs.layers[id].clone()
}

#[test]
fn scripts_read_inputs_and_write_layer_fields() {
    let mut script = load(
        r#"import { engine, props, audio, input, scene, timeline } from "wprs";
        const moon = scene.layer("moon");
        export function update({ time }) {
            moon.rotation = time * 10;
            moon.opacity = props.get("glow");
            moon.visible = input.cursor.inside;
            moon.position = [input.cursor.x + engine.dt * 0, input.cursor.y];
            moon.color = [audio.bass, 0, 0, 1];
            moon.text = `${audio.bands(16).length} ${audio.beat} ${input.cursor.buttons} ${engine.locale}`;
            timeline("intro").seek(2);
            timeline("intro").play();
            return time * 2;
        }"#,
    );
    let out = update(&mut script, 1.5).unwrap();
    let moon = LayerOverride {
        position: Some([0.25, 0.75]),
        rotation: Some(15.0),
        opacity: Some(0.5),
        visible: Some(true),
        color: Some([0.25, 0.0, 0.0, 1.0]),
        text: Some("16 true 1 en".into()),
        ..LayerOverride::default()
    };
    assert_eq!(layer(&out, "moon"), moon);
    let intro = [("intro".to_string(), TimelineCommand::Seek(2.0)), ("intro".into(), TimelineCommand::Play)];
    assert_eq!((out.timelines, out.values["main.js"].clone()), (intro.to_vec(), json!(3)));
}

#[test]
fn scripts_read_labels_colors_and_presses() {
    let labels = json!({ "mood": { "2": "Storm" } });
    let source = r#"import { props, scene } from "wprs";
        export function update() {
            const c = props.color("tint");
            scene.layer("sky").text = `${props.label("mood")} ${c.r} ${c.a} ${props.pressed("again")} ${props.color("mood")}`;
        }"#;
    let mut script = Script::load(&sources(source), 1, ("en", &labels), Limits::default()).unwrap();
    let props = [("mood".to_string(), json!(2)), ("tint".into(), json!("#ff000080"))].into();
    let (audio, pointer, actions) = (AudioFrame::default(), PointerState::default(), ["again".to_string()]);
    let frame =
        Frame { time: 0.0, dt: 0.0, props: &props, audio: &audio, pointer: &pointer, actions: &actions };
    let out = script.update(&frame).unwrap();
    let text = layer(&out, "sky").text.unwrap();
    assert_eq!(text, format!("Storm 1 {} true undefined", 128.0 / 255.0));
}

#[test]
fn every_file_of_a_scene_runs_in_order() {
    let files = [
        ("a.js", "import { scene } from 'wprs'; export function update() { scene.layer('x').opacity = 1; }"),
        (
            "b.js",
            "import { scene } from 'wprs'; export const update = () => (scene.layer('x').visible = false);",
        ),
        ("c.js", "import { scene } from 'wprs'; scene.layer('x').text = 'loaded';"),
    ];
    let files: Vec<_> = files.iter().map(|(p, s)| (p.to_string(), s.to_string())).collect();
    let mut script = Script::load(&files, 1, ("en", &json!({})), Limits::default()).unwrap();
    let out = update(&mut script, 0.0).unwrap();
    let x = LayerOverride { opacity: Some(1.0), visible: Some(false), ..LayerOverride::default() };
    assert_eq!((layer(&out, "x"), out.values.get("b.js")), (x, Some(&Value::Bool(false))));
    assert!(!out.values.contains_key("a.js") && !out.values.contains_key("c.js"));
}

#[test]
fn runaway_scripts_are_stopped_and_then_disabled() {
    let mut script = load("export function update() { for (;;) {} }");
    for _ in 0..3 {
        let start = Instant::now();
        assert_eq!(update(&mut script, 0.0), Err(Error::Timeout(Duration::from_millis(4))));
        assert!(start.elapsed() < Duration::from_millis(500), "{:?}", start.elapsed());
    }
    assert_eq!(update(&mut script, 0.0), Err(Error::Disabled));
    let looping = Script::load(&sources("for (;;) {}"), 1, ("en", &json!({})), Limits::default()).err();
    assert_eq!(looping, Some(Error::Timeout(Duration::from_millis(100))));
}

#[test]
fn native_work_and_huge_outputs_count_against_the_limits() {
    let mut sort = load("export function update() { new Float64Array(3e6).sort(); }");
    assert_eq!(update(&mut sort, 0.0), Err(Error::Timeout(Duration::from_millis(4))));
    let source = "import { scene } from 'wprs'; const t = 'x'.repeat(1e5); export function update() { scene.layer('a').text = t; }";
    assert!(matches!(update(&mut load(source), 0.0), Err(Error::Script(e)) if e.contains("bytes")));
}

#[test]
fn one_slow_frame_does_not_disable_a_script() {
    let mut script = load(
        r#"import { scene } from "wprs";
        export function update({ time }) { while (time < 1) {} scene.layer("a").opacity = 1; }"#,
    );
    assert!(matches!(update(&mut script, 0.0), Err(Error::Timeout(_))));
    assert_eq!(layer(&update(&mut script, 2.0).unwrap(), "a").opacity, Some(1.0));
}

#[test]
fn memory_and_stack_bombs_fail_cleanly() {
    let mut bomb =
        load("export function update() { const a = []; for (;;) a.push(new Array(1 << 16).fill(1.5)); }");
    assert_eq!(update(&mut bomb, 0.0), Err(Error::Memory));
    let mut deep = load("const f = (n) => f(n + 1) + 1; export function update() { f(0); }");
    assert!(matches!(update(&mut deep, 0.0), Err(Error::Script(e)) if e.contains("stack")));
}

#[test]
fn scripts_have_no_way_out() {
    let mut script = load(
        r#"import { scene } from "wprs";
        export function update() {
            const names = ["process", "require", "std", "os", "fetch", "XMLHttpRequest", "WebSocket", "__wprs"];
            scene.layer("a").text = names.filter((n) => typeof globalThis[n] !== "undefined").join();
        }"#,
    );
    assert_eq!(layer(&update(&mut script, 0.0).unwrap(), "a").text, Some(String::new()));
    for source in ["import * as fs from 'fs';", "import * as os from 'os';", "import x from './other.js';"] {
        assert!(
            matches!(
                Script::load(&sources(source), 1, ("en", &json!({})), Limits::default()),
                Err(Error::Script(_))
            ),
            "{source}"
        );
    }
    let mut unknown =
        load("import { scene } from 'wprs'; export function update() { scene.layer('a').src = 'x'; }");
    assert!(matches!(update(&mut unknown, 0.0), Err(Error::Script(e)) if e.contains("not a layer field")));
}

#[test]
fn random_numbers_follow_the_seed() {
    let source = "export function update() { return Math.random(); }";
    let values = |seed| {
        let mut script = Script::load(&sources(source), seed, ("en", &json!({})), Limits::default()).unwrap();
        (0..3).map(|t| update(&mut script, t as f64).unwrap().values["main.js"].clone()).collect::<Vec<_>>()
    };
    assert_eq!(values(1), values(1));
    assert_ne!(values(1), values(2));
}
