use super::*;

const SOURCE: &str = r#"import { scene, openShortcut } from "wprs";
    const asked = [];
    const tap = () => asked.push(openShortcut("open"), openShortcut("open"), openShortcut("site"));
    scene.layer("button").on("click", tap);
    scene.layer("button").on("down", () => asked.push(openShortcut("down")));
    export function update() {
        let error = "";
        try { asked.push(openShortcut("idle")); } catch (e) { error = e.message; }
        scene.layer("info").text = `${asked.join(",")} ${error}`;
    }"#;

fn scene(uses: serde_json::Value) -> serde_json::Value {
    json!({
        "scripts": [{ "path": "main.js", "uses": uses, "events": ["update", "click", "down"] }],
        "layers": [{ "id": "button", "type": "solid", "color": "#ffffff", "solid": true },
                   { "id": "info", "type": "text", "text": "", "size": 10 }]
    })
}

fn touch(kind: TouchKind) -> Touch {
    Touch { layer: "button".into(), kind, local: [1.0, 1.0], scene: [1.0, 1.0], bone: None }
}

#[test]
fn shortcuts_open_only_from_a_click_handler_once_a_frame_each() {
    let mut script = staged(scene(json!(["shortcuts"])), &[("main.js", SOURCE)]);
    let quiet = at(&mut script, 0.0).unwrap();
    assert!(quiet.open.is_empty());
    assert_eq!(text(&quiet, "info"), "false ");
    let touches = [touch(TouchKind::Down), touch(TouchKind::Click)];
    let out = script.update(&Frame { touches: &touches, ..Frame::at(0.1) }).unwrap();
    assert_eq!(out.open, ["open", "site"], "only the click asks, each property once");
    assert_eq!(text(&out, "info"), "false,false,true,true,true,false ");
    let after = at(&mut script, 0.2).unwrap();
    assert!(after.open.is_empty(), "a request lasts one frame");
}

#[test]
fn scripts_that_do_not_declare_shortcuts_cannot_open_them() {
    let mut script = staged(scene(json!([])), &[("main.js", SOURCE)]);
    let touches = [touch(TouchKind::Click)];
    let out = script.update(&Frame { touches: &touches, ..Frame::at(0.0) }).unwrap();
    assert!(out.open.is_empty());
    assert!(
        text(&out, "info").contains("main.js uses shortcuts without declaring it in scene.json"),
        "{}",
        text(&out, "info")
    );
}
