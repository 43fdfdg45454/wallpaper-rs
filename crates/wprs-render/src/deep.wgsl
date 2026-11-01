struct Deep { r: vec4<f32>, g: vec4<f32>, b: vec4<f32>, tone: vec4<f32>, m0: vec4<f32>, m1: vec4<f32>, m2: vec4<f32> };
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

fn pq(e: vec3<f32>) -> vec3<f32> {
    let p = pow(max(e, vec3(0.0)), vec3(1.0 / 78.84375));
    return 10000.0 * pow(max(p - 0.8359375, vec3(0.0)) / (18.8515625 - 18.6875 * p), vec3(1.0 / 0.1593017578125));
}

fn hlg(e: vec3<f32>) -> vec3<f32> {
    let high = (exp((e - 0.55991073) / 0.17883277) + 0.28466892) / 12.0;
    let lin = select(high, e * e / 3.0, e <= vec3(0.5));
    let y = dot(lin, vec3(0.2627, 0.6780, 0.0593));
    return 1000.0 * lin * pow(max(y, 1e-6), 0.2);
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let p = in.position.xy;
    let ycc = vec4(code(luma, vec2<i32>(p)), half(cb, p), half(cr, p), 1.0);
    let e = clamp(vec3(dot(d.r, ycc), dot(d.g, ycc), dot(d.b, ycc)), vec3(0.0), vec3(1.0));
    let light = select(pq(e), hlg(e), d.tone.x > 1.5) / d.tone.y;
    let c = max(vec3(dot(d.m0.xyz, light), dot(d.m1.xyz, light), dot(d.m2.xyz, light)), vec3(0.0));
    let k = d.tone.w;
    let l = max(max(c.r, c.g), c.b);
    let x = max(l - k, 0.0) / (1.0 - k);
    let wx = (d.tone.z / d.tone.y - k) / (1.0 - k);
    let mapped = select(l, k + (1.0 - k) * x * (1.0 + x / (wx * wx)) / (1.0 + x), l > k);
    return vec4(clamp(c * (mapped / max(l, 1e-6)), vec3(0.0), vec3(1.0)), 1.0);
}
