use super::*;

fn rows(params: &[f32]) -> Vec<[f32; 4]> {
    let mut fx = vec![[0.0; 4]; 64];
    params.iter().enumerate().for_each(|(k, v)| fx[k] = [*v; 4]);
    (fx[48], fx[50], fx[51], fx[52]) = (
        [1.0 / 64.0, 1.0 / 32.0, 64.0, 32.0],
        [2.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    );
    fx
}

fn shaped(id: &str, body: &str) -> &'static Descriptor {
    let json = json!({ "id": id, "mask_mode": "strength",
        "params": [{ "id": "strength", "type": "float", "default": 0.1 }, { "id": "flow", "type": "texture", "default": "" }],
        "passes": [{ "entry": "fs_p", "inputs": ["previous", "asset:flow"] }] });
    probe(json, &format!("@fragment\nfn fs_p(in: VertexOut) -> @location(0) vec4<f32> {{\n{body}\n}}\n"))
}

#[test]
fn the_farthest_read_of_an_effect_follows_its_parameters() {
    let flow = shaped(
        "probe_spread_flow",
        "    let lu = layer_uv(in.uv);\n    \
        let rg = (textureSampleLevel(aux0, samp, lu, 0.0).rg - vec2<f32>(0.5)) * 2.0;\n    \
        var o = sin(frame.time) * 0.5 + 0.5;\n    if param(1u).x > 0.5 {\n        o = o * 2.0 - 1.0;\n    }\n    \
        let s = param(0u).x;\n    let moved = from_layer(lu + o * s * s * rg);\n    \
        return mix(tap(in.uv), tap(moved), mask_at(moved));",
    );
    let near = flow.spread(&rows(&[0.1, 1.0]), true).unwrap();
    assert!((near[0] - 0.005).abs() < 1e-4 && (near[1] - 0.01).abs() < 1e-4, "{near:?}");
    let far = flow.spread(&rows(&[0.3, 1.0]), true).unwrap();
    assert!((far[1] - 0.09).abs() < 1e-3, "{far:?}");
    assert_eq!(flow.spread(&rows(&[0.1, 1.0]), false), None, "pictures that may hold any value");
    for (id, body) in [
        ("probe_spread_time", "    return tap(in.uv + vec2<f32>(frame.time, 0.0));"),
        ("probe_spread_scaled", "    return tap(in.uv * 2.0);"),
        ("probe_spread_raw", "    return textureSampleLevel(input_tex, samp, in.uv.yx, 0.0);"),
    ] {
        assert_eq!(shaped(id, body).spread(&rows(&[0.1, 1.0]), true), None, "{id}");
    }
    let own = shaped("probe_spread_own", "    return tap(in.uv) * mask_at(in.uv);");
    assert_eq!(own.spread(&rows(&[0.1, 1.0]), true), Some([0.0, 0.0]));
    let waves =
        descriptor("waves").unwrap().spread(&rows(&[4.0, 1.0, 0.0, 90.0, 1.0, 0.0, 0.0]), true).unwrap();
    assert!(waves[0] <= 4.0 / 64.0 + 1e-4 && waves[0] > 0.0, "{waves:?}");
}
