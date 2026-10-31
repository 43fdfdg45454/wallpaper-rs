use super::*;

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
    let setup = Setup { labels, ..Setup::plain(&sources(&[("main.js", source)])) };
    let mut script = Script::load(&setup, Limits::default()).unwrap();
    let props = [("mood".to_string(), json!(2)), ("tint".into(), json!("#ff000080"))].into();
    let actions = ["again".to_string()];
    let out = script.update(&Frame { props: &props, actions: &actions, ..Frame::at(0.0) }).unwrap();
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
    let mut script = plain(&files, 1).unwrap();
    let out = update(&mut script, 0.0).unwrap();
    let text = Some("loaded".to_string());
    let x = LayerOverride { opacity: Some(1.0), visible: Some(false), text, ..LayerOverride::default() };
    assert_eq!((layer(&out, "x"), out.values.get("b.js")), (x, Some(&Value::Bool(false))));
    assert!(!out.values.contains_key("a.js") && !out.values.contains_key("c.js"));
}
