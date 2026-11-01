use crate::allocations::allocations;
use crate::{Pixels, Renderer};
use image::{Rgba, RgbaImage};
use std::sync::Arc;
use wprs_scene::{Cut, Quality, Runtime, SCHEMA, Scene, Sketch, SketchBone, Spare, Values};

fn puppet() -> Scene {
    let mut s = Sketch::cut(&vec![255; 64 * 64], [64, 64], Cut::Grid, 16);
    let bone = |name: &str, parent, at| SketchBone { name: name.into(), parent, at };
    s.bones = vec![bone("root", None, [4.0, 32.0]), bone("arm", Some(0), [32.0, 32.0])];
    s.bind();
    let glb = s.glb();
    let doc = serde_json::json!({ "schema": SCHEMA, "canvas": { "size": [64, 64] },
        "assets": { "pic": { "path": "pic.png" }, "mesh": { "path": "mesh.glb" } },
        "layers": [{ "id": "fox", "type": "mesh", "mesh": "mesh", "texture": "pic",
            "clips": [{ "id": "swing", "loop": "pingpong", "bones": { "arm": { "rotation": [[0, 0], [1, 60]] } } }],
            "animations": [{ "clip": "swing" }] }] });
    let mut scene = Scene::from_json(&doc.to_string()).unwrap();
    scene.adopt(&Default::default(), move |_: &str, _: usize| Ok::<_, String>(glb.clone())).unwrap();
    scene
}

#[test]
fn a_moving_skeleton_poses_and_evaluates_its_frames_without_allocating() {
    let Ok(r) = Renderer::shared() else { return };
    let picture = RgbaImage::from_fn(64, 64, |x, y| Rgba([x as u8 * 4, y as u8 * 4, 90, 255]));
    let assets = super::super::SceneAssets {
        images: [("pic".to_string(), Pixels::new(picture))].into(),
        ..Default::default()
    };
    let mut runtime = Runtime::new(Arc::new(puppet()), Values::new());
    let mut view = r.scene_view(&runtime.scene, &assets, runtime.props()).unwrap();
    let mut spare = Spare::default();
    let mut counts = Vec::new();
    for k in 0..12 {
        let posed = allocations(|| runtime.advance(1.0 / 30.0));
        let mut frame = None;
        let kept = std::mem::take(&mut spare);
        let evaluated =
            allocations(|| frame = Some(runtime.frame_in(kept, view.view((64, 64)), Quality::Q3)));
        let frame = frame.unwrap();
        r.render_scene(&mut view, &runtime, &frame, (64, 64), Quality::Q3, &[]).unwrap();
        assert!(view.skins["fox"].generation >= k);
        spare = frame.spare();
        r.finish().unwrap();
        counts.push((posed, evaluated));
    }
    assert!(counts[4..].iter().all(|c| *c == (0, 0)), "{counts:?}");
}
