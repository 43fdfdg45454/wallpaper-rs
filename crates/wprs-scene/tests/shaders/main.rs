use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use wprs_scene::*;

mod adopt;
mod contract;

const WGSL: &str = r#"
override taps: i32 = 4;
override tinted: bool = false;

@fragment
fn fs_ripple_step(in: VertexOut) -> @location(0) vec4<f32> {
    let wave = sin(frame.time * 3.0 + in.uv.x * f32(taps));
    return tap(in.uv) * wave + sample(aux4, in.uv) * band_left(2u) + sample(aux7, in.uv) * 0.0;
}

@vertex
fn vs_ripple(@builtin(vertex_index) i: u32) -> VertexOut {
    let p = vec2<f32>(f32(i % 2u), f32(i / 2u));
    return VertexOut(vec4<f32>(rotate2(p * 2.0 - 1.0, 5.0), 0.0, 1.0), p);
}

@fragment
fn fs_ripple(in: VertexOut) -> @location(0) vec4<f32> {
    let c = tap(in.uv);
    let mixed = blend_mode(u32(param(2u).x), c.rgb, from_hsv(hsv(c.rgb)), 0.5) * greyscale(c.rgb);
    let tinted_rgb = select(mixed, mixed * layer_color().rgb, tinted);
    return vec4<f32>(tinted_rgb + vec3<f32>(tex_size(3u), daytime()) * 0.0 + band_of(1u, 16u) * 0.0, c.a * layer_alpha());
}
"#;

fn descriptor() -> Json {
    json!({ "id": "ripple", "name": { "en": "Ripple", "es": "Onda" },
        "params": [
            { "id": "detail", "type": "int", "default": 1, "min": 0, "max": 4, "label": { "en": "Detail", "es": "Detalle" } },
            { "id": "style", "type": "enum", "default": 0, "values": [{ "value": 0, "label": { "en": "Soft" } }, { "value": 1 }] },
            { "id": "mode", "type": "blend", "default": "screen", "help": { "en": "How the waves mix" } },
            { "id": "center", "type": "position", "default": [0.5, 0.5] },
            { "id": "source", "type": "texture", "default": "" },
            { "id": "paint", "type": "mask", "default": "", "channels": 3, "paint": "#000000" }
        ],
        "targets": { "history": { "scale": 0.5, "format": "rgba16f" } },
        "options": [{ "id": "taps", "values": [4, 8], "default": 4 },
                    { "id": "tinted", "values": [false, true], "default": false, "when_bound": "paint" }],
        "passes": [
            { "entry": "fs_ripple_step", "scale": 0.5, "target": "history", "blend": "add",
              "inputs": ["previous", "target:history", "backdrop", "asset:source", "asset:paint"] },
            { "copy": "target:history" },
            { "entry": "fs_ripple", "vertex": "vs_ripple", "grid": [16, 16], "inputs": ["input", "pass:1"] }
        ],
        "cost": { "taps": 4, "passes": 3, "scale": 1 } })
}

fn packaged(change: impl FnOnce(&mut Json), wgsl: &str) -> Result<Descriptor, String> {
    let mut d = descriptor();
    change(&mut d);
    Descriptor::packaged("ripple", &d.to_string(), wgsl)
}

fn scene(uses: Json) -> Scene {
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [64, 32] }, "assets": { "a": { "path": "a.png" } },
        "effects": { "ripple": { "descriptor": "fx/ripple.json", "wgsl": "fx/ripple.wgsl" } },
        "layers": [{ "id": "l", "type": "image", "image": "a", "effects": uses }] });
    Scene::load(&text.to_string(), &BTreeMap::new()).unwrap_or_else(|e| panic!("{e}"))
}

fn adopt(scene: &mut Scene, descriptor: &Json) -> Result<(), SceneError> {
    let files = |path: &str, _: usize| match path {
        "fx/ripple.json" => Ok(descriptor.to_string().into_bytes()),
        "fx/ripple.wgsl" => Ok(WGSL.as_bytes().to_vec()),
        _ => Err("missing"),
    };
    scene.adopt(&BTreeMap::new(), files)
}
