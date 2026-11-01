use crate::{Pixels, Renderer};
use image::{Rgba, RgbaImage};
use std::sync::Arc;
use wprs_scene::{Cut, Quality, Runtime, SCHEMA, Scene, Sketch};

fn grown(scale: f32, side: u32, quality: Quality) -> (u32, u32) {
    let Ok(r) = Renderer::shared() else { return (0, 0) };
    let glb = Sketch::cut(&vec![255; 64 * 64], [64, 64], Cut::Grid, 4).glb();
    let doc = serde_json::json!({ "schema": SCHEMA, "canvas": { "size": [64, 64] },
        "assets": { "pic": { "path": "pic.png" }, "mesh": { "path": "mesh.glb" } },
        "layers": [{ "id": "fox", "type": "mesh", "mesh": "mesh", "texture": "pic", "size": [64, 64],
            "transform": { "scale": [scale, scale] } }] });
    let mut scene = Scene::from_json(&doc.to_string()).unwrap();
    scene.adopt(&Default::default(), move |_: &str, _: usize| Ok::<_, String>(glb.clone())).unwrap();
    let picture = RgbaImage::from_pixel(side, side, Rgba([200, 100, 50, 255]));
    let assets = super::super::SceneAssets {
        images: [("pic".to_string(), Pixels::new(picture))].into(),
        ..Default::default()
    };
    let runtime = Runtime::new(Arc::new(scene), Default::default());
    let mut view = r.scene_view(&runtime.scene, &assets, runtime.props()).unwrap();
    let frame = runtime.frame(view.view((64, 64)), quality);
    r.render_scene(&mut view, &runtime, &frame, (64, 64), quality, &[]).unwrap();
    view.skins["fox"].image.size()
}

#[test]
fn a_scaled_up_mesh_is_skinned_at_the_pixels_it_covers_up_to_its_picture() {
    if Renderer::shared().is_err() {
        return;
    }
    assert_eq!(grown(1.0, 256, Quality::Q3), (64, 64));
    assert_eq!(grown(0.5, 256, Quality::Q3), (64, 64));
    assert_eq!(grown(2.0, 256, Quality::Q3), (128, 128));
    assert_eq!(grown(1.8, 256, Quality::Q3), (128, 128), "sizes grow by half octaves");
    assert_eq!(grown(1.3, 256, Quality::Q3), (91, 91));
    assert_eq!(grown(8.0, 256, Quality::Q3), (256, 256), "never past the picture");
    assert_eq!(grown(2.0, 64, Quality::Q3), (64, 64));
    assert_eq!(grown(2.0, 256, Quality::Q0), (64, 64));
}
