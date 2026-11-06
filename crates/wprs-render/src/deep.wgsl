struct Deep { r: vec4<f32>, g: vec4<f32>, b: vec4<f32>, tone: vec4<f32>, m0: vec4<f32>, m1: vec4<f32>, m2: vec4<f32>, light: vec4<f32> };
struct VertexOut { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };

@group(0) @binding(0) var<uniform> d: Deep;
@group(0) @binding(1) var luma: texture_2d<u32>;
@group(0) @binding(2) var cb: texture_2d<u32>;
@group(0) @binding(3) var cr: texture_2d<u32>;

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VertexOut {
    let q = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return VertexOut(vec4<f32>(q.x * 2.0 - 1.0, 1.0 - q.y * 2.0, 0.0, 1.0), q);
}

fn code(t: texture_2d<u32>, p: vec2<i32>) -> f32 {
    let last = vec2<i32>(textureDimensions(t)) - 1;
    return f32(textureLoad(t, clamp(p, vec2(0), last), 0).r) / 1023.0;
}

fn half(t: texture_2d<u32>, at: vec2<f32>) -> f32 {
    let p = at / 2.0 - 0.25;
    let base = vec2<i32>(floor(p));
    let f = fract(p);
    let top = mix(code(t, base), code(t, base + vec2(1, 0)), f.x);
    let bottom = mix(code(t, base + vec2(0, 1)), code(t, base + vec2(1, 1)), f.x);
    return mix(top, bottom, f.y);
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let p = in.position.xy;
    let ycc = vec4(code(luma, vec2<i32>(p)), half(cb, p), half(cr, p), 1.0);
    let e = clamp(vec3(dot(d.r, ycc), dot(d.g, ycc), dot(d.b, ycc)), vec3(0.0), vec3(1.0));
    if d.light.x > 1.0 {
        return vec4(lit(e, d.tone, d.m0.xyz, d.m1.xyz, d.m2.xyz, d.light.x), 1.0);
    }
    return vec4(mapped(e, d.tone, d.m0.xyz, d.m1.xyz, d.m2.xyz), 1.0);
}
