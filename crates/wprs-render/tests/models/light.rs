use super::*;

fn glowing() -> Scene {
    let doc = json!({ "schema": SCHEMA,
        "canvas": { "size": [SIDE, SIDE], "background": "#000000", "hdr": true, "tonemap": "aces" },
        "layers": [{ "id": "glow", "type": "solid", "color": "#ff4020", "brightness": 4 },
                   { "id": "mid", "type": "solid", "color": "#808080", "size": [16, 16] }] });
    Scene::from_json(&doc.to_string()).unwrap()
}

fn lit(r: &Renderer, headroom: f32) -> (Vec<[f32; 4]>, wgpu::TextureFormat) {
    let scene = glowing();
    let runtime = Runtime::new(Arc::new(scene.clone()), Values::new());
    let mut view = r.scene_view(&scene, &SceneAssets::default(), &Values::new()).unwrap();
    view.light(headroom);
    let frame = runtime.frame(view.view((SIDE, SIDE)), Quality::Q3);
    let image = r.render_scene(&mut view, &runtime, &frame, (SIDE, SIDE), Quality::Q3, &[]).unwrap();
    let format = image.format();
    (r.read_light(image).unwrap_or_default(), format)
}

#[test]
fn an_hdr_screen_gets_the_bright_colors_tone_mapped_to_its_peak() {
    let Some(r) = renderer() else { return };
    let (sdr, plain) = lit(r, 1.0);
    assert_eq!(plain, wgpu::TextureFormat::Rgba8UnormSrgb, "an SDR screen keeps the 8-bit picture");
    assert!(sdr.is_empty());
    let (four, wide) = lit(r, 4.0);
    assert_eq!(wide, wgpu::TextureFormat::Rgba16Float);
    let at = |px: &[[f32; 4]], x: u32, y: u32| px[(y * SIDE + x) as usize];
    let bright = at(&four, 4, 4);
    assert!(bright[0] > 1.5 && bright[0] < 4.0, "brighter than white, below the peak: {bright:?}");
    assert!(bright[0] > bright[1] && bright[1] > bright[2], "{bright:?}");
    let mid = at(&four, 32, 32);
    assert!((mid[0] - 0.2158).abs() < 0.01 && (mid[0] - mid[2]).abs() < 1e-3, "midtones stay: {mid:?}");
    let (two, _) = lit(r, 2.0);
    assert!(at(&two, 4, 4)[0] < bright[0] && at(&two, 4, 4)[0] < 2.0, "a dimmer peak bends more");
    let (wild, _) = lit(r, f32::NAN);
    assert!(wild.is_empty(), "a headroom that is not a number is SDR");
}
