use super::*;

fn single(id: &str, wgsl: &str) -> &'static Descriptor {
    probe(
        json!({ "id": id, "mask_mode": "strength", "passes": [{ "entry": "fs_p", "inputs": ["previous"] }] }),
        wgsl,
    )
}

#[test]
fn effects_that_read_only_their_own_pixel_are_told_apart_from_those_that_reach_further() {
    let near = "@fragment\nfn fs_p(in: VertexOut) -> @location(0) vec4<f32> {\n    let c = tap(in.uv);\n    \
        return mix(c, c * 0.5, mask_at(in.uv));\n}\n";
    let shifted = "@fragment\nfn fs_p(in: VertexOut) -> @location(0) vec4<f32> {\n    \
        return tap(in.uv + vec2<f32>(0.3, 0.0)) * mask_at(in.uv);\n}\n";
    let masked_away = "@fragment\nfn fs_p(in: VertexOut) -> @location(0) vec4<f32> {\n    \
        return tap(in.uv) * mask_at(in.uv * 0.5);\n}\n";
    let helper = "fn probe_far(uv: vec2<f32>) -> vec4<f32> {\n    return tap(uv);\n}\n@fragment\n\
        fn fs_p(in: VertexOut) -> @location(0) vec4<f32> {\n    return probe_far(in.uv) + probe_far(in.uv * 2.0);\n}\n";
    let direct = "@fragment\nfn fs_p(in: VertexOut) -> @location(0) vec4<f32> {\n    \
        return textureSampleLevel(input_tex, samp, in.uv.yx, 0.0);\n}\n";
    assert!(single("probe_near", near).reads_its_pixel());
    for (id, wgsl) in [
        ("probe_shifted", shifted),
        ("probe_masked_away", masked_away),
        ("probe_helper", helper),
        ("probe_direct", direct),
    ] {
        assert!(!single(id, wgsl).reads_its_pixel(), "{id}");
    }
    for far in ["perspective", "crt", "glitch", "edges"] {
        assert!(!descriptor(far).unwrap().reads_its_pixel(), "{far}");
    }
    for near in ["tint", "light_shafts", "lens_flare"] {
        assert!(descriptor(near).unwrap().reads_its_pixel(), "{near}");
    }
}

#[test]
fn effects_that_read_their_mask_only_under_their_pixel_are_told_apart() {
    let shifted = "@fragment\nfn fs_p(in: VertexOut) -> @location(0) vec4<f32> {\n    \
        return tap(in.uv + vec2<f32>(0.3, 0.0)) * mask_at(in.uv);\n}\n";
    let masked_away = "@fragment\nfn fs_p(in: VertexOut) -> @location(0) vec4<f32> {\n    \
        return tap(in.uv) * mask_at(in.uv * 0.5);\n}\n";
    let direct = "@fragment\nfn fs_p(in: VertexOut) -> @location(0) vec4<f32> {\n    \
        return tap(in.uv) * textureSampleLevel(mask_tex, samp, in.uv, 0.0).r;\n}\n";
    assert!(single("probe_mask_shifted", shifted).reads_mask_at_its_pixel());
    assert!(!single("probe_mask_away", masked_away).reads_mask_at_its_pixel());
    assert!(!single("probe_mask_direct", direct).reads_mask_at_its_pixel());
    for near in ["waves", "tint", "perspective"] {
        assert!(descriptor(near).unwrap().reads_mask_at_its_pixel(), "{near}");
    }
}
