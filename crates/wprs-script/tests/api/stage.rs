use super::*;

const SOURCE: &str = r##"import { scene } from "wprs";
    export function update({ time }) {
        try {
            scene.camera = { zoom: 2, shake: { amplitude: 4 } };
            scene.camera = { rotation: 15, shake: { speed: 3 } };
            scene.parallax = { amount: 0.5 };
            scene.bloom = { strength: 2, tint: "#ff0000" };
            scene.background = "#102030";
            scene.ambient = "#202020";
            scene.sky = [0.5, 0.6, 1, 1];
            if (time > 0) scene.camera = null;
            scene.layer("info").text = JSON.stringify(scene.camera);
            scene.camera = { zoom: "near" };
        } catch (e) {
            scene.layer("info").text += " " + e.message;
        }
    }"##;

fn scene(uses: serde_json::Value) -> serde_json::Value {
    json!({
        "scripts": [{ "path": "main.js", "uses": uses }],
        "layers": [{ "id": "info", "type": "text", "text": "", "size": 10 }]
    })
}

#[test]
fn scripts_move_the_camera_and_change_the_scene_settings() {
    let mut script = staged(scene(json!(["camera"])), &[("main.js", SOURCE)]);
    let out = at(&mut script, 0.0).unwrap();
    let stage = out.stage.unwrap();
    let camera = stage.camera.unwrap();
    assert_eq!((camera.zoom, camera.rotation), (Some(2.0), Some(15.0)));
    let shake = camera.shake.unwrap();
    assert_eq!((shake.amplitude, shake.speed), (Some(4.0), Some(3.0)));
    assert_eq!(stage.parallax.unwrap().amount, Some(0.5));
    assert_eq!(
        (stage.bloom.unwrap().strength, stage.bloom.unwrap().tint),
        (Some(2.0), Some([1.0, 0.0, 0.0, 1.0]))
    );
    let [r, g, b, _] = stage.background.unwrap();
    assert!(
        (r - 16.0 / 255.0).abs() < 1e-6 && (g - 32.0 / 255.0).abs() < 1e-6 && (b - 48.0 / 255.0).abs() < 1e-6
    );
    assert_eq!(
        (stage.ambient.map(|c| c[0] > 0.12 && c[0] < 0.13), stage.sky),
        (Some(true), Some([0.5, 0.6, 1.0, 1.0]))
    );
    assert!(text(&out, "info").ends_with(" zoom takes a number"), "{}", text(&out, "info"));
    let later = at(&mut script, 0.1).unwrap();
    assert!(text(&later, "info").starts_with("{} zoom"), "{}", text(&later, "info"));
    assert!(later.stage.is_some_and(|s| s.camera.is_none() && s.parallax.is_some()));
    let bounded = wprs_scene::StageSet {
        camera: Some(wprs_scene::CameraSet { zoom: Some(99.0), ..Default::default() }),
        ..Default::default()
    };
    assert_eq!(bounded.bounded().camera.unwrap().zoom, Some(10.0));
    let lit = wprs_scene::StageSet {
        ambient: Some([2.0, f32::NAN, 0.5, 1.0]),
        sky: Some([2.0, 0.0, 0.5, 1.0]),
        ..Default::default()
    };
    assert_eq!((lit.bounded().ambient, lit.bounded().sky), (None, Some([1.0, 0.0, 0.5, 1.0])));
}

#[test]
fn scripts_that_do_not_declare_camera_leave_the_scene_settings() {
    let mut script = staged(scene(json!(["timers"])), &[("main.js", SOURCE)]);
    let out = at(&mut script, 0.0).unwrap();
    assert!(out.stage.is_none());
    assert!(
        text(&out, "info").contains("main.js uses camera without declaring it in scene.json"),
        "{}",
        text(&out, "info")
    );
}
