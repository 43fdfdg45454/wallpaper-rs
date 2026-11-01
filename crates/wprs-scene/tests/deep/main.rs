use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use wprs_scene::*;

mod parse;
mod schema;

const DEEP: &str = include_str!("../deep.json");
const GLASS: &str = "fn material(s: Surface) -> Surface {\n    var o = s;\n    o.albedo = vec4<f32>(o.albedo.rgb, 0.4);\n    return o;\n}";
const SPARKLE: &str = "fn particle(p: Sprite) -> Sprite {\n    var o = p;\n    o.size = p.size * (1.0 + sin(frame.time));\n    return o;\n}";

const EXPECTED: [&str; 34] = [
    "attach",
    "capture",
    "lights",
    "blend_shapes",
    "boids",
    "bone_physics",
    "bone_rules",
    "camera3d",
    "ik",
    "layer_events",
    "material_shaders",
    "media_info",
    "mesh",
    "model3d",
    "particle_children",
    "particle_collisions",
    "particle_depth",
    "particle_fields",
    "particle_layers",
    "particle_lighting",
    "particle_looks",
    "particle_ropes",
    "particle_shaders",
    "particle_steering",
    "particles2",
    "rgb",
    "script_geometry",
    "script_layers",
    "script_shortcuts",
    "script_storage",
    "scripts2",
    "shadows",
    "timeline_events",
    "custom_effects",
];

pub fn glb(doc: &Json) -> Vec<u8> {
    let mut text = doc.to_string().into_bytes();
    text.resize(text.len().div_ceil(4) * 4, b' ');
    let mut out = b"glTF".to_vec();
    for word in [2, 20 + text.len() as u32, text.len() as u32, 0x4E4F_534A] {
        out.extend(word.to_le_bytes());
    }
    out.extend(text);
    out
}

pub fn figure() -> Json {
    let names = ["root", "arm", "hand", "hair", "sword", "head", "socket"];
    json!({ "asset": { "version": "2.0" },
        "nodes": names.iter().map(|n| if *n == "arm" { json!({ "name": n, "children": [2] }) } else { json!({ "name": n }) }).collect::<Vec<_>>(),
        "skins": [{ "joints": [0, 1, 2, 3, 4, 5] }],
        "meshes": [
            { "name": "body", "extras": { "targetNames": ["mouth_open", "cheeks", "blink"] },
              "primitives": [{ "attributes": { "POSITION": 0, "JOINTS_0": 1, "WEIGHTS_0": 2 }, "indices": 3, "targets": [{}, {}, {}] }] },
            { "name": "arm", "primitives": [{ "attributes": { "POSITION": 0 } }] }],
        "accessors": [{ "count": 300 }, { "count": 300 }, { "count": 300 }, { "count": 600 },
                      { "count": 2, "max": [2.0] }, { "count": 2, "max": [3.5] }],
        "animations": [{ "name": "idle", "samplers": [{ "input": 4 }] }, { "name": "wave", "samplers": [{ "input": 5 }] }],
        "materials": [{ "name": "metal" }, { "name": "hair" }, { "name": "glass" }] })
}

fn files(model: &Json) -> impl FnMut(&str, usize) -> Result<Vec<u8>, &'static str> + '_ {
    move |path: &str, max: usize| match path {
        p if p.ends_with(".glb") => Some(glb(model)).filter(|b| b.len() <= max).ok_or("too large"),
        "effects/glass.json" => {
            Ok(json!({ "id": "glass", "stage": "material", "cost": {} }).to_string().into_bytes())
        }
        "effects/sparkle.json" => {
            Ok(json!({ "id": "sparkle", "stage": "particle", "cost": {} }).to_string().into_bytes())
        }
        "effects/glass.wgsl" => Ok(GLASS.as_bytes().to_vec()),
        "effects/sparkle.wgsl" => Ok(SPARKLE.as_bytes().to_vec()),
        _ => Err("missing"),
    }
}

fn adopt(scene: &mut Scene, model: &Json) -> Result<(), SceneError> {
    scene.adopt(&BTreeMap::new(), files(model))
}

fn deep() -> Scene {
    Scene::load(DEEP, &BTreeMap::new()).unwrap_or_else(|e| panic!("{e}"))
}
