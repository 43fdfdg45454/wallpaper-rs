use serde_json::{Value as Json, json};
use wprs_scene::*;

mod limits;
mod loops;

fn shader(body: &str) -> String {
    format!(
        "override taps: u32 = 4u;\noverride neg: i32 = -1;\nfn helper(p: ptr<function, u32>) {{ *p = 1u; }}\n@fragment\nfn fs_w(in: VertexOut) -> @location(0) vec4<f32> {{\n    var c = tap(in.uv);\n{body}\n    return c;\n}}"
    )
}

fn effect(change: impl FnOnce(&mut Json), body: &str) -> Result<Descriptor, String> {
    let mut d = json!({ "id": "w", "cost": {}, "passes": [{ "entry": "fs_w" }],
        "options": [{ "id": "taps", "values": [4, 16], "default": 4 }] });
    change(&mut d);
    Descriptor::packaged("w", &d.to_string(), &shader(body))
}
