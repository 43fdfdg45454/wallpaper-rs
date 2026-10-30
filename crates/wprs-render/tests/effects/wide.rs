use super::budget::packaged;
use super::*;

fn moving_package_over_still_blurs(still: usize) {
    let blurred = |j| {
        json!({ "id": format!("s{j}"), "type": "solid", "color": "#0000ff", "opacity": 0.02,
            "effects": [{ "effect": "blur", "params": { "radius": 2 } }] })
    };
    let mut layers: Vec<Value> = (0..still).map(blurred).collect();
    let position = json!({ "keys": [[0, [1910, 1080]], [1, [1930, 1080]]], "loop": "pingpong" });
    layers.push(json!({ "id": "top", "type": "solid", "color": "#00ff00", "size": [3072, 1728],
        "transform": { "position": position }, "effects": [{ "effect": "pk" }] }));
    let json = json!({ "id": "pk", "cost": {}, "passes": [{ "entry": "fs_m" }] });
    let wgsl = "@fragment fn fs_m(in: VertexOut) -> @location(0) vec4<f32> { return vec4<f32>(1.0, 0.0, 1.0, 1.0) * tap(in.uv).a; }";
    let Some(mut run) = packaged(&json, wgsl, layers) else { return };
    for k in 0..4 {
        run.runtime.advance(1.0 / 60.0);
        let shot = run.shot_at((7680, 4320));
        let held = run.view.texture_bytes() >> 20;
        assert_eq!(shot.at(3840, 2160), [255, 0, 255], "frame {k} with {still} blurred layers, {held} MiB");
        assert!(run.view.left_out().is_empty(), "{:?}", run.view.left_out());
    }
}

#[test]
fn a_moving_package_effect_keeps_drawing_on_an_8k_output() {
    moving_package_over_still_blurs(0);
    moving_package_over_still_blurs(3);
}
