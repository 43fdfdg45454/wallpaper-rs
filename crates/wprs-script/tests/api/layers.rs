use super::*;

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
            if (n === 2) scene.layer("c").position[0];
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
    assert_eq!(first.watch, ["a"], "a layer that is only written is not read back");
    let second = script.update(&Frame { reads: &reads, ..Frame::at(0.1) }).unwrap();
    assert_eq!(second.watch, ["a", "c"]);
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
