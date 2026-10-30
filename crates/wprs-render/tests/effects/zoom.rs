use super::budget::packaged;
use super::*;

#[test]
fn a_zooming_blurred_layer_holds_a_bounded_set_of_textures() {
    let scale = json!({ "keys": [[0, 0.1], [1, 1.95]], "loop": "pingpong" });
    let layer = json!({ "id": "z", "type": "solid", "color": "#ff0000", "size": [1920, 1080],
        "transform": { "position": [1920, 1080], "scale": scale },
        "effects": [{ "effect": "blur", "params": { "radius": 4 } }] });
    let json = json!({ "id": "unused", "cost": {}, "passes": [{ "entry": "fs_u" }] });
    let wgsl = "@fragment fn fs_u(in: VertexOut) -> @location(0) vec4<f32> { return tap(in.uv); }";
    let Some(mut run) = packaged(&json, wgsl, vec![layer]) else { return };
    let (size, frames) = if run.r.software() { ((960, 540), 40) } else { ((3840, 2160), 200) };
    let (mut most, mut textures) = (0, 0);
    for _ in 0..frames {
        run.runtime.advance(1.0 / 60.0);
        let frame = run.runtime.frame(run.view.view(size), run.quality);
        run.r.render_scene(&mut run.view, &run.runtime, &frame, size, run.quality, &[]).unwrap();
        (most, textures) = (most.max(run.view.texture_bytes()), textures.max(run.view.stats().textures));
    }
    run.r.finish().unwrap();
    assert!(most <= 1 << 30, "the view held {} MiB of textures", most >> 20);
    assert!(textures < 128, "{textures} textures");
}
