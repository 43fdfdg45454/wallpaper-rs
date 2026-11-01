use super::*;

const SOURCE: &str = r#"import { scene } from "wprs";
    import { lookAt } from "wprs/math";
    const [moon, sun, kid] = ["moon", "sun", "kid"].map((id) => scene.layer(id));
    export function update({ time }) {
        const said = [];
        const step = (f) => { try { said.push(f()); } catch (e) { said.push(e.message); } };
        step(() => String(moon.parent));
        step(() => kid.parent);
        step(() => sun.children.join("+"));
        if (time > 0) {
            step(() => (moon.setParent("sun", true), "moved"));
            step(() => moon.parent);
            step(() => sun.setParent("moon"));
            step(() => moon.world.decompose().translation.toArray().join(","));
        }
        step(() => Math.round(lookAt([0, 0], [0, 10])));
        scene.layer("info").text = said.join("|");
    }"#;

fn scene(uses: serde_json::Value) -> serde_json::Value {
    let solid = |id: &str, at: [f32; 2]| json!({ "id": id, "type": "solid", "color": "#ffffff", "transform": { "position": at } });
    json!({
        "scripts": [{ "path": "main.js", "uses": uses }],
        "layers": [solid("sun", [50.0, 20.0]), solid("moon", [70.0, 30.0]),
                   { "id": "pack", "type": "group", "layers": [solid("kid", [1.0, 1.0])] },
                   { "id": "info", "type": "text", "text": "", "size": 10 }]
    })
}

fn read(id: &str, world: [f32; 6]) -> (String, LayerRead) {
    (id.into(), LayerRead { world, ..LayerRead::default() })
}

#[test]
fn scripts_read_and_change_the_parents_of_layers() {
    let mut script = staged(scene(json!(["hierarchy", "math"])), &[("main.js", SOURCE)]);
    assert_eq!(text(&at(&mut script, 0.0).unwrap(), "info"), "null|pack||90");
    let reads =
        [read("sun", [2.0, 0.0, 0.0, 2.0, 50.0, 20.0]), read("moon", [1.0, 0.0, 0.0, 1.0, 70.0, 30.0])];
    let out = script.update(&Frame { reads: &reads, ..Frame::at(0.1) }).unwrap();
    let said = text(&out, "info");
    assert!(
        said.starts_with("null|pack||moved|sun|sun cannot follow a layer that follows it|70,30|90"),
        "{said}"
    );
    let moon = &out.layers["moon"];
    assert_eq!(
        (moon.parent.as_deref(), moon.position, moon.scale),
        (Some("sun"), Some([10.0, 5.0]), Some([0.5, 0.5]))
    );
}

#[test]
fn scripts_that_do_not_declare_hierarchy_cannot_reach_parents() {
    let mut script = staged(scene(json!(["math"])), &[("main.js", SOURCE)]);
    let said = text(&at(&mut script, 0.1).unwrap(), "info");
    assert!(said.starts_with("main.js uses hierarchy without declaring it in scene.json"), "{said}");
    assert!(said.ends_with("|90"), "{said}");
}
