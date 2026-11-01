use super::*;

fn shot_with_video(r: &Renderer, shot: &mut Shot, video: &wprs_render::GpuImage) -> Vec<u8> {
    let frame = shot.runtime.frame(shot.view.view(SIZE), Quality::Q3);
    let videos = [("clip", video)];
    let image = r.render_scene(&mut shot.view, &shot.runtime, &frame, SIZE, Quality::Q3, &videos).unwrap();
    let mut target = r.create_target(SIZE.0, SIZE.1).unwrap();
    let layer = Layer {
        image: Some(image),
        fit: Fit::Stretch,
        background: [0; 3],
        opacity: 1.0,
        uv: None,
        relief: None,
    };
    let mut pixels = Vec::new();
    r.render_layers(&mut target, &[layer], &mut pixels).unwrap();
    pixels
}

#[test]
fn videos_feed_pictures_masks_and_effect_textures() {
    let Some(r) = renderer() else { return };
    let white_left =
        RgbaImage::from_fn(8, 4, |x, _| if x < 4 { Rgba([255; 4]) } else { Rgba([0, 0, 0, 255]) });
    let video = r.upload_rgba(white_left.as_raw(), 8, 4, 32).unwrap();
    let layers = json!([
        { "id": "pic", "type": "image", "image": "clip", "size": [50, 50], "transform": { "position": [25, 25] } },
        { "id": "red", "type": "solid", "color": "#ff0000", "size": [50, 50], "transform": { "position": [75, 25] },
          "mask": { "texture": "clip" } },
        { "id": "tex", "type": "adjustment", "effects": [{ "effect": "blend_texture",
          "params": { "texture": "clip", "blend": "multiply", "amount": 1 } }], "visible": false }
    ]);
    let tinted = json!({ "canvas": canvas("#ff0000"), "assets": { "clip": { "path": "clip.webm" } },
        "layers": [{ "id": "tex", "type": "adjustment", "effects": [{ "effect": "blend_texture",
          "params": { "texture": "clip", "blend": "multiply", "amount": 1 } }] }] });
    let textured = shot_with_video(r, &mut Shot::with(r, tinted, &[]), &video);
    check(&textured, &[((10, 25), RED), ((90, 25), [0; 3])]);
    let scene = json!({ "canvas": canvas("#000000"), "assets": { "clip": { "path": "clip.webm" } }, "layers": layers });
    let mut shot = Shot::with(r, scene, &[]);
    let pixels = shot_with_video(r, &mut shot, &video);
    check(&pixels, &[((10, 25), [255; 3]), ((40, 25), [0; 3]), ((60, 25), RED), ((90, 25), [0; 3])]);
    let view = shot.view.view(SIZE);
    assert!(shot.runtime.frame(view, Quality::Q3).draws[0].dynamic, "a playing video moves");
    let paused = json!({ "canvas": canvas("#000000"), "assets": { "clip": { "path": "clip.webm" } },
        "layers": [{ "id": "v", "type": "video", "video": "clip", "playback": { "playing": false } }] });
    let mut still = Shot::with(r, paused, &[]);
    still.runtime.advance(0.1);
    assert!(!still.runtime.frame(view, Quality::Q3).draws[0].dynamic, "a paused video stands still");
    let hidden = json!({ "canvas": canvas("#000000"), "assets": { "clip": { "path": "clip.webm" } },
        "layers": [{ "id": "v", "type": "video", "video": "clip", "visible": false }] });
    let mut unseen = Shot::with(r, hidden, &[]);
    unseen.runtime.frame(view, Quality::Q3);
    unseen.runtime.advance(1.0);
    assert!(
        !unseen.runtime.tapes()[0].shown && unseen.runtime.tapes()[0].at == 0.0,
        "nobody sees it, so it waits"
    );
}
