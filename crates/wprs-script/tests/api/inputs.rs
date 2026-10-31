use super::*;

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

#[test]
fn shared_values_are_copied_frozen_and_counted_however_they_are_set() {
    let source = r#"import { shared, scene } from "wprs";
        const errors = [];
        const kept = [1, 2];
        shared.kept = kept;
        kept.push(3);
        const tries = [
            () => { shared.list = []; shared.list.push(1); },
            () => { shared.f = () => 1; },
            () => { shared.deep = { a: new Array(70000).fill(0) }; },
            () => { for (let i = 0; i < 300; i++) Object.defineProperty(shared, `k${i}`, { value: i, enumerable: true }); },
            () => Object.defineProperty(shared, "k0", { get: () => 1 }),
            () => Object.setPrototypeOf(shared, null),
        ];
        for (const f of tries) { try { f(); } catch (e) { errors.push(e.name); } }
        export function update() { scene.layer("a").text = `${errors} ${Object.keys(shared).length} ${shared.kept.length} ${shared.list.length}`; }"#;
    let mut script = load(source);
    let out = at(&mut script, 0.0).unwrap();
    assert!(out.stopped.is_empty(), "{:?}", out.stopped);
    assert_eq!(text(&out, "a"), "TypeError,TypeError,RangeError,RangeError,TypeError,TypeError 256 2 0");
}

#[test]
fn copies_of_templates_stay_within_the_layer_limit() {
    let children: Vec<_> =
        (0..100).map(|i| json!({ "id": format!("t{i}"), "type": "solid", "color": "#ffffff" })).collect();
    let scene = json!({
        "scripts": [{ "path": "main.js", "uses": ["layers"] }],
        "templates": { "big": { "id": "big", "type": "group", "layers": children } },
        "layers": [{ "id": "info", "type": "text", "text": "", "size": 10 }]
    });
    let source = r#"import { scene } from "wprs";
        let made = 0, error = "";
        try { for (;;) { scene.spawn("big"); made += 1; } } catch (e) { error = e.message; }
        export function update() { scene.layer("info").text = `${made} ${error}`; }"#;
    let mut script = staged(scene, &[("main.js", source)]);
    let out = at(&mut script, 0.0).unwrap();
    assert_eq!(text(&out, "info"), "5 the scene and the layers added to it hold at most 512 layers");
    assert_eq!(out.spawned.len(), 5);
}
