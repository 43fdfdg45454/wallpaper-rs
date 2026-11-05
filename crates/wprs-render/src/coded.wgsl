struct Coded { r: vec4<f32>, g: vec4<f32>, b: vec4<f32>, turn: vec4<f32>, tone: vec4<f32>, m0: vec4<f32>, m1: vec4<f32>, m2: vec4<f32> };
struct VertexOut { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };

@group(0) @binding(0) var<uniform> d: Coded;
@group(0) @binding(1) var luma: texture_2d<f32>;
@group(0) @binding(2) var chroma: texture_2d<f32>;
@group(0) @binding(3) var linear: sampler;

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VertexOut {
    let q = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return VertexOut(vec4<f32>(q.x * 2.0 - 1.0, 1.0 - q.y * 2.0, 0.0, 1.0), q);
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let ycc = vec4(textureSample(luma, linear, in.uv).r, textureSample(chroma, linear, in.uv).rg, 1.0);
    let e = clamp(vec3(dot(d.r, ycc), dot(d.g, ycc), dot(d.b, ycc)), vec3(0.0), vec3(1.0));
    return vec4(mapped(e, d.tone, d.m0.xyz, d.m1.xyz, d.m2.xyz), 1.0);
}
