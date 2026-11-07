use crate::{Pixels, Renderer};
use image::{Rgba, RgbaImage};
use std::sync::Arc;
use wprs_scene::{Cut, Quality, Runtime, SCHEMA, Scene, Sketch, SketchBone, Values};

fn skins(layers: &[&str]) -> Vec<RgbaImage> {
    let Ok(r) = Renderer::shared() else { return Vec::new() };
    let mut s = Sketch::cut(&vec![255; 64 * 64], [64, 64], Cut::Grid, 16);
    let bone = |name: &str, parent, at| SketchBone { name: name.into(), parent, at };
    s.bones = vec![bone("root", None, [4.0, 32.0]), bone("arm", Some(0), [32.0, 32.0])];
    s.bind();
    let glb = s.glb();
    let layer = |id: &&str| {
        serde_json::json!({ "id": id, "type": "mesh", "mesh": "mesh", "texture": id,
        "clips": [{ "id": "swing", "loop": "pingpong", "bones": { "arm": { "rotation": [[0, 0], [1, 60]] } } }],
        "animations": [{ "clip": "swing" }] })
    };
    let doc = serde_json::json!({ "schema": SCHEMA, "canvas": { "size": [64, 64] },
        "assets": { "a": { "path": "a.png" }, "b": { "path": "b.png" }, "c": { "path": "c.png" },
            "mesh": { "path": "mesh.glb" } },
        "layers": layers.iter().map(layer).collect::<Vec<_>>() });
    let mut scene = Scene::from_json(&doc.to_string()).unwrap();
    scene.adopt(&Default::default(), move |_: &str, _: usize| Ok::<_, String>(glb.clone())).unwrap();
    let picture = |k: u8| {
        let alpha = |x: u32, y: u32| if (x + y * k as u32) % 7 < 2 { 0 } else { (x * 4) as u8 | 3 };
        RgbaImage::from_fn(64, 64, |x, y| Rgba([x as u8 * 4, y as u8 * k, 90, alpha(x, y)]))
    };
    let images = ["a", "b", "c"].iter().zip(1..).map(|(n, k)| (n.to_string(), Pixels::new(picture(k))));
    let assets = super::super::SceneAssets { images: images.collect(), ..Default::default() };
    let mut runtime = Runtime::new(Arc::new(scene), Values::new());
    runtime.advance(0.3);
    let mut view = r.scene_view(&runtime.scene, &assets, runtime.props()).unwrap();
    let frame = runtime.frame(view.view((64, 64)), Quality::Q3);
    r.render_scene(&mut view, &runtime, &frame, (64, 64), Quality::Q3, &[]).unwrap();
    layers.iter().map(|l| r.read_image(&view.skins[*l].image).unwrap()).collect()
}

#[test]
fn meshes_of_one_pose_drawn_in_one_pass_match_each_drawn_alone() {
    let together = skins(&["a", "b", "c"]);
    if together.is_empty() {
        return;
    }
    for (k, id) in ["a", "b", "c"].iter().enumerate() {
        assert!(together[k] == skins(&[id])[0], "the mesh {id} changed when drawn with the others");
    }
}
