struct Blur { texel: vec2<f32>, offset: f32, pad: f32 };
struct VertexOut { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };

@group(0) @binding(0) var<uniform> b: Blur;
@group(0) @binding(1) var t: texture_2d<f32>;
@group(0) @binding(2) var smp: sampler;

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VertexOut {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return VertexOut(vec4<f32>(p.x * 2.0 - 1.0, 1.0 - p.y * 2.0, 0.0, 1.0), p);
}

fn tap(uv: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(t, smp, uv, 0.0);
}

@fragment
fn fs_down(in: VertexOut) -> @location(0) vec4<f32> {
    let h = b.texel * b.offset;
    let d = vec2(h.x, -h.y);
    return (tap(in.uv) * 4.0 + tap(in.uv - h) + tap(in.uv + h) + tap(in.uv + d) + tap(in.uv - d)) / 8.0;
}

@fragment
fn fs_up(in: VertexOut) -> @location(0) vec4<f32> {
    let h = b.texel * b.offset;
    let x = vec2(h.x * 2.0, 0.0);
    let y = vec2(0.0, h.y * 2.0);
    let e = tap(in.uv - x) + tap(in.uv + x) + tap(in.uv - y) + tap(in.uv + y);
    let c = tap(in.uv + h) + tap(in.uv - h) + tap(in.uv + vec2(h.x, -h.y)) + tap(in.uv - vec2(h.x, -h.y));
    return (e + c * 2.0) / 12.0;
}
