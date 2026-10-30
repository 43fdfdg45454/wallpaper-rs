mod common;

use common::{at, load, staged, text};
use serde_json::json;
use wprs_scene::{AudioFrame, Extent, LayerRead, PointerState, Values, script_key};
use wprs_script::{Frame, Mode, Touch, TouchKind};

fn spin_scene() -> serde_json::Value {
    let params = json!({ "speed": { "type": "number", "default": 1, "min": 0, "max": 10 } });
    let spin = json!({ "script": "spin.js", "fn": "spin", "params": { "speed": 2 } });
    let fade = json!({ "script": "spin.js", "fn": "fade" });
    json!({
        "scripts": [{ "path": "spin.js", "events": ["init"], "params": params }],
        "layers": [{ "id": "dot", "type": "solid", "color": "#ffffff", "opacity": fade, "transform": { "rotation": spin } }]
    })
}

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

#[test]
fn layers_read_their_values_and_keep_what_scripts_write() {
    let mut script = load(
        r##"import { scene } from "wprs";
        const a = scene.layer("a");
        let n = 0;
        export function update() {
            n += 1;
            if (n === 1) {
                a.skew = [5, 0]; a.origin = { x: 0, y: 1 }; a.parallax = 0.5; a.size = 24; a.font = "serif"; a.align = "left";
                a.color = "#ff0000"; a.scale = 2;
                scene.layer("b").text = `${a.position} ${a.rotation} ${a.visible} ${a.opacity} ${a.text} ${a.skew}`;
            }
            if (n === 3) a.reset("size");
            try { a.align = "middle"; } catch (e) { scene.layer("b").text = e.message; }
        }"##,
    );
    let read = LayerRead {
        position: [10.0, 20.0],
        rotation: 45.0,
        visible: true,
        opacity: 0.5,
        text: Some("hi".into()),
        size: Some(Extent::Points(12.0)),
        ..LayerRead::default()
    };
    let reads = [("a".to_string(), read)];
    let first = script.update(&Frame { reads: &reads, ..Frame::at(0.0) }).unwrap();
    let a = &first.layers["a"];
    assert_eq!(
        (a.skew, a.origin, a.parallax, a.size),
        (Some([5.0, 0.0]), Some([0.0, 1.0]), Some([0.5, 0.5]), Some(Extent::Points(24.0)))
    );
    assert_eq!(
        (a.font.as_deref(), a.color, a.scale),
        (Some("serif"), Some([1.0, 0.0, 0.0, 1.0]), Some([2.0, 2.0]))
    );
    assert_eq!(text(&first, "b"), "align takes left, center or right");
    assert_eq!(first.watch, ["a", "b"]);
    script.update(&Frame { reads: &reads, ..Frame::at(0.1) }).unwrap();
    let third = script.update(&Frame { reads: &reads, ..Frame::at(0.2) }).unwrap();
    assert_eq!((third.layers["a"].size, third.layers["a"].skew), (None, Some([5.0, 0.0])));
}

#[test]
fn effects_are_switched_and_tuned_within_their_descriptor() {
    let source = r#"
        import { scene } from "wprs";
        const soft = scene.layer("pic").effect("soft");
        soft.enabled = false;
        soft.set("radius", 1000);
        let message = "";
        try { soft.set("glow", 1); } catch (e) { message = e.message; }
        try { scene.layer("pic").effect(4); } catch (e) { message += ` ${e.message}`; }
        export function update() { soft.reset(); scene.layer("pic").text = `${message} ${soft.get("radius")} ${soft.effect} ${soft.index}`; }
    "#;
    let scene = json!({
        "scripts": [{ "path": "main.js", "uses": ["effects"] }],
        "layers": [{ "id": "pic", "type": "solid", "color": "#808080", "effects": [{ "effect": "blur", "name": "soft" }] }]
    });
    let mut script = staged(scene, &[("main.js", source)]);
    let out = at(&mut script, 0.0).unwrap();
    let control = &out.effects["pic"][0];
    assert_eq!(
        (control.at, control.enabled, control.params["radius"].clone(), control.reset),
        (0, Some(false), wprs_scene::Param::Number(128.0), 1)
    );
    assert_eq!(text(&out, "pic"), "blur has no parameter glow pic has no effect 4 128 blur 0");
}

#[test]
fn templates_spawn_destroy_and_order_layers() {
    let source = r#"
        import { scene } from "wprs";
        const bars = [0, 1, 2].map((i) => scene.spawn("bar", { position: [i * 10, 0] }));
        const named = scene.spawn("bar", { id: "first", at: 0, opacity: 0.5 });
        bars[1].destroy();
        scene.order("back", 3);
        const errors = [];
        for (const f of [() => scene.spawn("nope"), () => scene.spawn("bar", { id: "back" }), () => scene.spawn("bar", { id: "a b" }), () => scene.layer("back").destroy()]) {
            try { f(); } catch (e) { errors.push(e.constructor.name); }
        }
        let room = 0;
        try { for (;;) { scene.spawn("bar"); room += 1; } } catch (e) { errors.push(e.constructor.name); }
        export function update() { scene.layer("info").text = `${scene.layers().slice(0, 5)} ${errors} ${room} ${named.initial.type} ${named.type}`; }
    "#;
    let scene = json!({
        "scripts": [{ "path": "main.js", "uses": ["layers"] }],
        "templates": { "bar": { "id": "bar", "type": "solid", "color": "#ffffff", "size": [8, 40] } },
        "layers": [{ "id": "back", "type": "solid", "color": "#000000" }, { "id": "info", "type": "text", "text": "", "size": 10 }]
    });
    let mut script = staged(scene, &[("main.js", source)]);
    let out = at(&mut script, 0.0).unwrap();
    assert_eq!(
        text(&out, "info"),
        "first,info,bar_1,back,bar_3 TypeError,TypeError,TypeError,TypeError,RangeError 253 solid solid"
    );
    assert_eq!(out.spawned.len(), 256);
    assert_eq!(out.spawned[0], ("bar_1".to_string(), "bar".to_string()));
    assert_eq!(
        out.order.as_ref().map(|o| o[..4].to_vec()),
        Some(vec!["first".into(), "info".into(), "bar_1".into(), "back".into()])
    );
    assert_eq!((out.layers["bar_3"].position, out.layers["first"].opacity), (Some([20.0, 0.0]), Some(0.5)));
}

#[test]
fn modules_share_values_and_import_each_other() {
    let files = [
        (
            "scripts/a.js",
            "import { shared } from 'wprs'; import { double } from './lib/b.js'; shared.n = double(21);",
        ),
        ("scripts/lib/b.js", "export const double = (x) => x * 2;"),
        (
            "scripts/c.js",
            r#"import { shared, scene } from "wprs";
            let error = "";
            try { for (let i = 0; i < 300; i++) shared[`k${i}`] = i; } catch (e) { error = e.message; }
            try { shared.k0 = "x".repeat(70000); } catch (e) { error += ` / ${e.message}`; }
            export function update() { scene.layer("a").text = `${shared.n} ${error}`; }"#,
        ),
    ];
    let mut script =
        wprs_script::Script::load(&wprs_script::Setup::plain(&common::sources(&files)), Default::default())
            .unwrap();
    let out = at(&mut script, 0.0).unwrap();
    assert_eq!(
        text(&out, "a"),
        "42 shared holds at most 256 values / a shared value holds at most 65536 items"
    );
}

#[test]
fn console_lines_are_counted_and_cut() {
    let mut script = load(
        r#"export function update({ time }) {
            if (time === 0) for (let i = 0; i < 30; i++) console.log("line", i, { i });
            else console.warn("y".repeat(2000));
        }"#,
    );
    let first = at(&mut script, 0.0).unwrap();
    assert_eq!((first.console.len(), first.console[1].as_str()), (20, "log main.js: line 1 {\"i\":1}"));
    let second = script.update(&Frame { wall: 5.0, ..Frame::at(1.0) }).unwrap();
    assert_eq!((second.console.len(), second.console[0].len()), (1, 512));
    assert!(second.console[0].starts_with("warn main.js: yyy"));
}

#[test]
fn a_failing_module_stops_alone() {
    let files = [
        (
            "bad.js",
            "import { setInterval } from 'wprs'; setInterval(() => {}, 20); export function update() { throw new Error('boom'); }",
        ),
        (
            "good.js",
            "import { scene } from 'wprs'; export function update({ time }) { scene.layer('a').rotation = time; }",
        ),
    ];
    let mut script =
        wprs_script::Script::load(&wprs_script::Setup::plain(&common::sources(&files)), Default::default())
            .unwrap();
    let outs: Vec<_> = (0..4).map(|t| at(&mut script, f64::from(t)).unwrap()).collect();
    assert!(outs[0].console.iter().any(|l| l == "error bad.js: Error: boom"));
    assert_eq!(
        (outs[1].stopped.len(), outs[2].stopped.as_slice()),
        (0, ["bad.js: Error: boom".to_string()].as_slice())
    );
    assert_eq!((outs[3].layers["a"].rotation, outs[3].wake), (Some(3.0), None));
}

#[test]
fn the_cursor_and_layer_touches_reach_scripts() {
    let mut script = load(
        r#"import { scene, input } from "wprs";
        const log = [];
        scene.layer("button").on("click", (e) => log.push(`${e.type} ${e.layer.id} ${e.local} ${e.scene}`));
        let message = "";
        try { scene.layer("button").on("hover", () => {}); } catch (e) { message = e.message; }
        export function update() {
            const c = input.cursor;
            scene.layer("a").text = `${c.scene} ${c.screen} ${c.left} ${c.inside} | ${log.join(";")} | ${message}`;
        }"#,
    );
    let pointer =
        PointerState { pos: Some([0.5, 0.25]), buttons: 1, inside: true, ..PointerState::default() };
    let touches =
        [Touch { layer: "button".into(), kind: TouchKind::Click, local: [1.0, 2.0], scene: [3.0, 4.0] }];
    let frame = Frame {
        pointer: &pointer,
        view: [100.0, 0.0, 200.0, 100.0],
        screen: [400, 200],
        touches: &touches,
        ..Frame::at(0.0)
    };
    let out = script.update(&frame).unwrap();
    assert_eq!(
        text(&out, "a"),
        "200,25 200,50 true true | click button 1,2 3,4 | on takes click, enter, leave, move, down, up and a function"
    );
}

#[test]
fn the_math_module_handles_vectors_matrices_and_colors() {
    let mut script = load(
        r#"import { scene } from "wprs";
        import { Vec2, Vec3, Vec4, Mat3, Mat4, smoothstep, mix, clamp, angleVector, vectorAngle, rgbToHsv, hsvToRgb, remap, angleDelta } from "wprs/math";
        const r = (x) => Math.round(x * 1000) / 1000;
        const v = new Vec2(3, 4);
        const m = Mat3.translation(10, 0).multiply(Mat3.rotation(90));
        const p = m.transformPoint([1, 0]);
        const inv = m.invert().multiply(m);
        const look = Mat4.translation(1, 2, 3).multiply(Mat4.scaling(2));
        const d = look.decompose();
        const lines = [
            v.length(), v.normalize().toArray().map(r), v.add([1, 1]).toArray(), v.dot({ x: 1, y: 0 }), r(v.rotate(90).x), v.cross([0, 1]),
            new Vec3(1, 0, 0).cross([0, 1, 0]).toArray(), new Vec4(1, 2, 3, 4).scale(2).toArray(), new Vec2(1, 1).reflect([0, 1]).toArray(),
            [r(p.x), r(p.y)], inv.equals(Mat3.identity()), r(m.determinant()), r(m.decompose().rotation),
            d.translation.toArray(), d.scale.toArray(), r(Mat4.fromEuler(0, 0, 90).decompose().rotation.z),
            smoothstep(0, 1, 0.5), mix(0, 10, 0.25), mix([0, 0], [2, 4], 0.5), clamp(3), r(vectorAngle(angleVector(30))),
            rgbToHsv([1, 0, 0]), hsvToRgb([1 / 3, 1, 1]), remap(5, 0, 10, 100, 200), angleDelta(350, 10), JSON.stringify(v),
        ];
        export function update() { scene.layer("a").text = lines.join(" | "); }"#,
    );
    let out = at(&mut script, 0.0).unwrap();
    let want = "5 | 0.6,0.8 | 4,5 | 3 | -4 | 3 | 0,0,1 | 2,4,6,8 | 1,-1 | 10,1 | true | 1 | 90 | 1,2,3 | 2,2,2 | 90 | 0.5 | 2.5 | 1,2 | 1 | 30 | 0,1,1 | 0,1,0 | 150 | 20 | [3,4]";
    assert_eq!(text(&out, "a"), want);
}

#[test]
fn modules_only_use_the_parts_they_declare() {
    let source = r#"
        import { scene, setTimeout } from "wprs";
        const tried = [];
        for (const f of [() => scene.spawn("bar"), () => scene.layer("a").effect(0), () => setTimeout(() => {}, 20)]) {
            try { f(); tried.push("ok"); } catch (e) { tried.push(e.message); }
        }
        export function update() { scene.layer("a").text = tried.join(" | "); }
    "#;
    let scene = json!({
        "scripts": [{ "path": "main.js", "uses": ["timers"] }],
        "templates": { "bar": { "id": "bar", "type": "solid", "color": "#ffffff" } },
        "layers": [{ "id": "a", "type": "solid", "color": "#000000", "effects": [{ "effect": "blur" }] }]
    });
    let out = at(&mut staged(scene, &[("main.js", source)]), 0.0).unwrap();
    let refused = "main.js uses layers without declaring it in scene.json | main.js uses effects without declaring it in scene.json | ok";
    assert_eq!(text(&out, "a"), refused);
}
