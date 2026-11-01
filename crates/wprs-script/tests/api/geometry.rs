use super::*;

const SOURCE: &str = r#"import { scene } from "wprs";
    let bars = null, shape = null;
    const said = [];
    const step = (f) => { try { said.push(f()); } catch (e) { said.push(e.message); } };
    step(() => (bars = scene.mesh("bars"), "pool"));
    export function update({ time }) {
        if (bars && time === 0) {
            step(() => (shape = bars.create({ vertices: new Float32Array([0, 0, 10, 0, 0, 10]), format: "xy" }), shape.id));
            step(() => bars.create({ vertices: [0, 0, 1], format: "xy" }));
            step(() => bars.create({ vertices: [0, 0, 1, 1, 2, 2], indices: [0, 1], format: "xy" }));
            step(() => bars.create({ vertices: [0, 0, 1, 1, 0, 0, 1, 1, 2, 2, 1, 1], indices: [0, 1, 2], format: "xyuv" }).id);
            step(() => bars.create({ vertices: [0, 0, 1, 1, 2, 2], format: "xy" }));
            step(() => scene.mesh("nope"));
        }
        if (bars && time > 0.15) step(() => shape.update({ vertices: [5, 5, 6, 6, 7, 7] }));
        scene.layer("info").text = said.join("|");
    }"#;

fn scene(uses: serde_json::Value) -> serde_json::Value {
    json!({
        "scripts": [{ "path": "main.js", "uses": uses }],
        "pools": { "bars": { "type": "mesh", "max": 2, "max_vertices": 64 } },
        "layers": [{ "id": "bars", "type": "video", "video": "bars" }, { "id": "info", "type": "text", "text": "", "size": 10 }]
    })
}

#[test]
fn scripts_build_shapes_in_their_pools_and_send_only_changes() {
    let mut script = staged(scene(json!(["geometry"])), &[("main.js", SOURCE)]);
    let out = at(&mut script, 0.0).unwrap();
    let said = text(&out, "info");
    let wanted = "pool|1|xy vertices hold 2 numbers each|indices come in threes|2|bars holds at most 2 shapes|nope is not a pool of the scene";
    assert_eq!(said, wanted);
    let mesh = out.meshes["bars"].clone();
    assert_eq!((mesh.shapes, mesh.count(), mesh.indices.clone()), (2, 6, vec![0, 1, 2, 3, 4, 5]));
    assert_eq!(&mesh.vertices[..8], &[0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]);
    assert_eq!(&mesh.vertices[24..32], &[0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]);
    let idle = at(&mut script, 0.1).unwrap();
    assert_eq!(idle.meshes["bars"].generation, mesh.generation, "an unchanged pool sends nothing");
    let moved = at(&mut script, 0.2).unwrap();
    assert!(moved.meshes["bars"].generation > mesh.generation && moved.meshes["bars"].vertices[0] == 5.0);
    let mut plain = staged(scene(json!(["timers"])), &[("main.js", SOURCE)]);
    let out = at(&mut plain, 0.0).unwrap();
    assert!(
        out.meshes.is_empty() && text(&out, "info").starts_with("main.js uses geometry without declaring it")
    );
}
