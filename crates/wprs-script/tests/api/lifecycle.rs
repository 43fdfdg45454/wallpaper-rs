use super::*;

#[test]
fn bindings_call_their_function_with_the_last_value_and_their_params() {
    let source = r#"
        export const spin = (value, ctx) => (value ?? 0) + ctx.params.speed;
        export const fade = {
            init: (value, ctx) => (ctx.layer === "dot" && ctx.path === "/layers/0/opacity" ? 0.25 : -1),
            update: (value, ctx) => value + ctx.params.speed / 4,
        };
    "#;
    let mut script = staged(spin_scene(), &[("spin.js", source)]);
    let spin =
        script_key("spin.js", Some("spin"), Some(&[("speed".into(), wprs_scene::Param::Number(2.0))].into()));
    let fade = script_key("spin.js", Some("fade"), None);
    let first = at(&mut script, 0.0).unwrap();
    assert_eq!(
        (first.values[&spin].clone(), first.values[&fade].clone(), first.busy),
        (json!(2), json!(0.5), true)
    );
    let second = at(&mut script, 0.1).unwrap();
    assert_eq!((second.values[&spin].clone(), second.values[&fade].clone()), (json!(4), json!(0.75)));
}

#[test]
fn modules_hear_their_lifecycle_and_only_wake_for_events() {
    let source = r#"
        import { scene, engine } from "wprs";
        const log = [];
        const out = scene.layer("out");
        const write = () => (out.text = log.join(" "));
        export function init(ctx) { log.push(`init:${ctx.params.speed}`); write(); }
        export function properties(changed) { log.push(`props:${Object.keys(changed).join(",")}`); write(); }
        export function settings(s) { log.push(`settings:${s.fps}/${s.quality}`); write(); }
        export function resize({ screen, canvas }) { log.push(`resize:${screen}/${canvas}/${engine.orientation}`); write(); }
    "#;
    let params = json!({ "speed": { "type": "number", "default": 3 } });
    let scene = json!({
        "scripts": [{ "path": "main.js", "events": ["init", "properties", "settings", "resize"], "params": params }],
        "layers": [{ "id": "out", "type": "text", "text": "", "size": 10 }]
    });
    let mut script = staged(scene, &[("main.js", source)]);
    let one: Values = [("a".to_string(), json!(1)), ("b".into(), json!(2))].into();
    let two: Values = [("a".to_string(), json!(1)), ("b".into(), json!(5))].into();
    let first = script.update(&Frame { props: &one, ..Frame::at(0.0) }).unwrap();
    assert_eq!(text(&first, "out"), "init:3 props:a,b settings:60/3");
    assert!(!first.busy);
    let next = script.update(&Frame { props: &two, screen: [100, 300], fps: 30, ..Frame::at(1.0) }).unwrap();
    assert_eq!(
        text(&next, "out"),
        "init:3 props:a,b settings:60/3 resize:100,300/200,100/portrait props:b settings:30/3"
    );
    script.destroy();
}

#[test]
fn the_engine_tells_its_context_without_names_of_outputs() {
    let mut script = load(
        r#"import { scene, engine } from "wprs";
        export function update() {
            scene.layer("a").text = [engine.canvas, engine.screen, engine.device, engine.mode, engine.orientation, engine.fps, engine.quality, engine.language].join(" ");
        }"#,
    );
    let frame =
        Frame { screen: [2560, 1440], mode: Mode::Lockscreen, fps: 144, quality: 2, ..Frame::at(0.0) };
    let out = script.update(&frame).unwrap();
    assert_eq!(text(&out, "a"), "1920,1080 2560,1440 desktop lockscreen landscape 144 2 ");
}

#[test]
fn timers_follow_the_scene_clock_once_a_frame() {
    let mut script = load(
        r#"import { scene, setTimeout, setInterval, clearTimeout } from "wprs";
        const log = [];
        setTimeout(() => log.push("once"), 100);
        const cancel = setTimeout(() => log.push("never"), 50);
        setInterval(() => log.push("tick"), 1);
        clearTimeout(cancel);
        let errors = 0;
        for (let i = 0; i < 70; i++) try { setTimeout(() => {}, 1e9); } catch (e) { errors += e instanceof RangeError; }
        export function update() { scene.layer("a").text = `${log.join(",")} ${errors}`; }"#,
    );
    let first = at(&mut script, 0.0).unwrap();
    assert_eq!((text(&first, "a"), first.wake), (" 8".into(), Some(0.016)));
    assert_eq!(text(&at(&mut script, 0.05).unwrap(), "a"), "tick 8");
    let late = at(&mut script, 5.0).unwrap();
    assert_eq!(text(&late, "a"), "tick,tick,once 8");
    assert!((late.wake.unwrap() - 0.016).abs() < 1e-9);
}

#[test]
fn audio_buffers_fill_in_place() {
    let mut script = load(
        r#"import { scene, audio } from "wprs";
        const b = audio.buffers(16);
        const same = audio.buffers(16) === b;
        export function update() { scene.layer("a").text = `${same} ${b.left[0]} ${b.right[15]} ${b.average.length} ${audio.buffers(64).left[63]}`; }"#,
    );
    let mut audio = AudioFrame::default();
    audio.left[2] = 0.5;
    audio.right[63] = 0.25;
    audio.left[63] = 0.75;
    let out = script.update(&Frame { audio: Some(&audio), ..Frame::at(0.0) }).unwrap();
    assert_eq!(text(&out, "a"), "true 0.5 0.25 16 0.75");
    audio.left[2] = 0.125;
    let out = script.update(&Frame { audio: Some(&audio), ..Frame::at(0.1) }).unwrap();
    assert_eq!(text(&out, "a"), "true 0.125 0.25 16 0.75");
}
