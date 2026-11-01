use crate::{Cut, Runtime, SCHEMA, Scene, Sketch, SketchBone, Values};
use std::sync::Arc;

fn waving(repeat: &str) -> Runtime {
    let mut s = Sketch::cut(&vec![255; 64 * 64], [64, 64], Cut::Grid, 4);
    let bone = |name: &str, parent, at| SketchBone { name: name.into(), parent, at };
    s.bones = vec![bone("root", None, [4.0, 32.0]), bone("arm", Some(0), [32.0, 32.0])];
    s.bind();
    let glb = s.glb();
    let doc = serde_json::json!({ "schema": SCHEMA, "canvas": { "size": [64, 64] },
        "assets": { "mesh": { "path": "mesh.glb" } },
        "layers": [{ "id": "fox", "type": "mesh", "mesh": "mesh",
            "clips": [{ "id": "c", "loop": repeat, "bones": { "arm": { "rotation": [[0, 0], [1, 90]] } } }],
            "animations": [{ "clip": "c" }] }] });
    let mut scene = Scene::from_json(&doc.to_string()).unwrap();
    scene.adopt(&Default::default(), move |_: &str, _: usize| Ok::<_, String>(glb.clone())).unwrap();
    Runtime::new(Arc::new(scene), Values::new())
}

#[test]
fn clip_time_stays_exact_after_days_of_looping() {
    for repeat in ["repeat", "pingpong"] {
        let (mut far, mut near) = (waving(repeat), waving(repeat));
        far.puppets[0].heads[0].t = f64::from(1u32 << 20) * 2.0 + 0.25;
        near.puppets[0].heads[0].t = 0.25;
        far.advance(1.0 / 60.0);
        near.advance(1.0 / 60.0);
        let (a, b) = (&far.puppets[0].heads[0], &near.puppets[0].heads[0]);
        assert!((a.t - b.t).abs() < 1e-6 && a.t < 2.0, "{repeat}: {} {}", a.t, b.t);
        let (pa, pb) = (&far.puppets[0].posed.palette, &near.puppets[0].posed.palette);
        let same = pa.iter().flatten().zip(pb.iter().flatten()).all(|(x, y)| (x - y).abs() < 1e-4);
        assert!(same, "{repeat}");
    }
}
