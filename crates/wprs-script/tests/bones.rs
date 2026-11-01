mod common;

use common::{sources, text};
use serde_json::json;
use std::collections::BTreeMap;
use wprs_scene::{BoneSet, Cut, LayerRead, Scene, Sketch, SketchBone};
use wprs_script::{Frame, Limits, Script, Setup, Touch, TouchKind};

fn glb() -> Vec<u8> {
    let mut s = Sketch::cut(&vec![255; 64 * 64], [64, 64], Cut::Grid, 4);
    let bone = |name: &str, parent, at| SketchBone { name: name.into(), parent, at };
    s.bones = vec![
        bone("root", None, [8.0, 32.0]),
        bone("arm", Some(0), [32.0, 32.0]),
        bone("hand", Some(1), [56.0, 32.0]),
    ];
    s.shapes = vec![("smile".into(), s.points.iter().map(|p| [p[0], p[1] + 1.0]).collect())];
    s.bind();
    s.glb()
}

fn rigged(uses: &[&str], source: &str) -> (Script, Vec<String>) {
    let doc = json!({ "schema": wprs_scene::SCHEMA, "canvas": { "size": [200, 100] }, "assets": { "rig": { "path": "fox.glb" } },
        "scripts": [{ "path": "main.js", "uses": uses, "events": ["down"] }],
        "layers": [{ "id": "a", "type": "text", "text": "", "size": 10, "font": "serif" },
            { "id": "fox", "type": "mesh", "mesh": "rig", "size": [64, 64], "hit": ["arm"] }] });
    let mut scene = Scene::from_json(&doc.to_string()).unwrap();
    let bytes = glb();
    scene.adopt(&BTreeMap::new(), move |_: &str, _: usize| Ok::<_, String>(bytes.clone())).unwrap();
    let names = scene.rigs["rig"].joints.iter().map(|j| j.name.clone()).collect();
    let setup = Setup::new(&scene, &sources(&[("main.js", source)]), 1, ("en", &json!({})));
    (Script::load(&setup, Limits::default()).unwrap(), names)
}

fn frames(names: &[String]) -> Vec<[f32; 6]> {
    let frame = |n: &str| match n {
        "root" => [1.0, 0.0, 0.0, 1.0, 8.0, 32.0],
        "arm" => [0.0, 1.0, -1.0, 0.0, 32.0, 32.0],
        "hand" => [0.0, 1.0, -1.0, 0.0, 32.0, 56.0],
        _ => [1.0, 0.0, 0.0, 1.0, 32.0, 32.0],
    };
    names.iter().map(|n| frame(n)).collect()
}

const SOURCE: &str = r#"import { scene } from "wprs";
    const fox = scene.layer("fox");
    let heard = "none";
    fox.on("down", (e) => { heard = e.bone; });
    const r = (v) => Math.round(v * 1000) / 1000;
    export function update(f) {
        const arm = fox.bone("arm");
        const a = fox.attachment("hand");
        scene.layer("a").text = [fox.bones.filter((b) => b !== "node_0").join(","), r(arm.rotation), arm.position.map(r), arm.worldPosition, r(arm.worldRotation),
            fox.hit(30, 50), fox.hit(12, 30), a.origin.map(r), r(a.angle), a.uv.map(r), heard].join(" ");
        if (f.time === 0) {
            arm.rotation = -30;
            fox.bone("hand").worldPosition = [40, 40];
            fox.bone("root").scale = 2;
            fox.shape("smile", 0.5);
            fox.impulse("arm", [10, 0]);
        }
        if (f.time === 2) fox.bone("arm").reset();
    }"#;

#[test]
fn scripts_read_and_drive_bones_shapes_and_attachments() {
    let (mut script, names) = rigged(&["animations", "layers", "clicks"], SOURCE);
    let read = LayerRead {
        world: [1.0, 0.0, 0.0, 1.0, 100.0, 50.0],
        origin: [0.5, 0.5],
        bones: frames(&names),
        ..LayerRead::default()
    };
    let reads = [("fox".to_string(), read)];
    let touches = [Touch {
        layer: "fox".into(),
        kind: TouchKind::Down,
        local: [32.0, 40.0],
        scene: [0.0; 2],
        bone: Some("arm".into()),
    }];
    script.update(&Frame { reads: &reads, ..Frame::at(0.0) }).unwrap();
    let out = script.update(&Frame { reads: &reads, touches: &touches, ..Frame::at(1.0) }).unwrap();
    let listed = names.iter().filter(|n| *n != "node_0").cloned().collect::<Vec<_>>().join(",");
    assert_eq!(text(&out, "a"), format!("{listed} -30 0,0 32,32 90 arm arm 100,74 90 0.5,0.875 arm"));
    let rig = &out.rigs["fox"];
    assert_eq!(rig.bones["arm"], BoneSet { rotation: Some(-30.0), ..BoneSet::default() });
    assert_eq!(
        rig.bones["hand"],
        BoneSet { position: Some([40.0, 40.0]), world: true, ..BoneSet::default() }
    );
    assert_eq!(rig.bones["root"].scale, Some([2.0, 2.0]));
    assert_eq!(rig.shapes["smile"], 0.5);
    assert!(rig.impulses.is_empty(), "an impulse lasts one frame");
    let out = script.update(&Frame { reads: &reads, ..Frame::at(2.0) }).unwrap();
    assert!(!out.rigs["fox"].bones.contains_key("arm"));
}

#[test]
fn an_impulse_reaches_the_engine_once() {
    let (mut script, _) = rigged(
        &["animations"],
        r#"import { scene } from "wprs";
        export function update(f) { if (f.time === 0) scene.layer("fox").impulse("hand", { x: 3, y: 4 }); }"#,
    );
    let out = script.update(&Frame::at(0.0)).unwrap();
    assert_eq!(out.rigs["fox"].impulses, [("hand".to_string(), [3.0, 4.0])]);
    let out = script.update(&Frame::at(1.0)).unwrap();
    assert!(out.rigs.get("fox").is_none_or(|r| r.impulses.is_empty()));
}

#[test]
fn bones_need_the_animations_part_and_known_names() {
    let (mut script, _) = rigged(
        &["layers"],
        r#"import { scene } from "wprs";
        export function update() { scene.layer("fox").bone("arm").rotation = 10; }"#,
    );
    let out = script.update(&Frame::at(0.0)).unwrap();
    let said = [out.console, out.stopped].concat().join(" ");
    assert!(said.contains("uses animations without declaring it"), "{said}");
    assert!(out.rigs.values().all(|r| r.bones.is_empty()));
    let (mut script, _) = rigged(
        &["layers"],
        r#"import { scene } from "wprs";
        export function update() { scene.layer("a").text = scene.layer("fox").bones.join(","); }"#,
    );
    let out = script.update(&Frame::at(0.0)).unwrap();
    assert_eq!(text(&out, "a"), "");
    let said = [out.console, out.stopped].concat().join(" ");
    assert!(said.contains("uses animations without declaring it"), "{said}");
    let (mut script, _) = rigged(
        &["animations", "layers"],
        r#"import { scene } from "wprs";
        let said = "";
        try { scene.layer("fox").bone("tail"); } catch (e) { said = e.message; }
        try { scene.layer("fox").shape("frown", 1); } catch (e) { said += ` | ${e.message}`; }
        try { scene.layer("a").bone; } catch (e) { said += " | nothing"; }
        export function update() { scene.layer("a").text = `${said} | ${scene.layer("a").bone === undefined}`; }"#,
    );
    let out = script.update(&Frame::at(0.0)).unwrap();
    assert_eq!(text(&out, "a"), "fox has no bone tail | fox has no blend shape frown | true");
}
