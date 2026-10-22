struct Op { kind: u32, mask: u32, mode: u32, opacity: f32, a: vec4<f32>, m: mat3x3<f32> };
struct Mask { kind: u32, invert: u32, channel: u32, pad: u32, a: vec4<f32>, b: vec4<f32> };
struct Stack { geo: mat3x3<f32>, start: u32, end: u32, aspect: f32, ratio: f32, ops: array<Op, 24>, masks: array<Mask, 8> };
struct VertexOut { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };

@group(0) @binding(0) var<uniform> s: Stack;
@group(0) @binding(1) var src: texture_2d<f32>;
@group(0) @binding(2) var blr: texture_2d<f32>;
@group(0) @binding(3) var lut: texture_2d<f32>;
@group(0) @binding(4) var aux: texture_2d<f32>;
@group(0) @binding(5) var dep: texture_2d<f32>;
@group(0) @binding(6) var smp: sampler;

const LUMA = vec3<f32>(0.2126, 0.7152, 0.0722);

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VertexOut {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return VertexOut(vec4<f32>(p.x * 2.0 - 1.0, 1.0 - p.y * 2.0, 0.0, 1.0), p);
}

fn enc(c: vec3<f32>) -> vec3<f32> {
    let x = max(c, vec3(0.0));
    return select(1.055 * pow(x, vec3(1.0 / 2.4)) - 0.055, x * 12.92, x <= vec3(0.0031308));
}

fn dec(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3(2.4)), c / 12.92, c <= vec3(0.04045));
}

fn at(x: f32, y: f32) -> vec4<f32> {
    let n = f32(textureDimensions(lut).x);
    return textureSampleLevel(lut, smp, vec2((clamp(x, 0.0, 1.0) * (n - 1.0) + 0.5) / n, y), 0.0);
}

fn curve(c: vec3<f32>, row: u32) -> vec3<f32> {
    let y = (f32(row) + 0.5) / f32(textureDimensions(lut).y);
    let m = vec3(at(c.r, y).a, at(c.g, y).a, at(c.b, y).a);
    return vec3(at(m.r, y).r, at(m.g, y).g, at(m.b, y).b);
}

fn blend(a: vec3<f32>, b: vec3<f32>, mode: u32) -> vec3<f32> {
    switch mode {
        case 1u: { return a * b; }
        case 2u: { return 1.0 - (1.0 - a) * (1.0 - b); }
        case 3u: { return select(1.0 - 2.0 * (1.0 - a) * (1.0 - b), 2.0 * a * b, a < vec3(0.5)); }
        case 4u: { return (1.0 - 2.0 * b) * a * a + 2.0 * b * a; }
        case 5u: { return clamp(b + dot(a - b, LUMA), vec3(0.0), vec3(1.0)); }
        default: { return b; }
    }
}

fn band(x: f32, r: vec4<f32>) -> f32 {
    return smoothstep(r.x - r.z, r.x, x) * (1.0 - smoothstep(r.y, r.y + r.z, x));
}

fn mask(i: u32, p: vec2<f32>, c: vec3<f32>) -> f32 {
    if i == 0u { return 1.0; }
    let k = s.masks[i - 1u];
    let w = vec2(s.aspect, 1.0);
    var v = 1.0;
    switch k.kind {
        case 1u: { let d = (k.a.zw - k.a.xy) * w; v = smoothstep(0.0, 1.0, dot((p - k.a.xy) * w, d) / max(dot(d, d), 1e-6)); }
        case 2u: { v = 1.0 - smoothstep(1.0 - k.b.x, 1.0, length((p - k.a.xy) / max(k.a.zw, vec2(1e-4)))); }
        case 3u: { v = band(dot(enc(c), LUMA), k.a); }
        case 4u: { v = textureSampleLevel(aux, smp, p, 0.0)[k.channel]; }
        case 5u: { v = band(textureSampleLevel(dep, smp, p, 0.0).r, k.a); }
        case 6u: { return 0.0; }
        default: {}
    }
    return select(v, 1.0 - v, k.invert == 1u);
}

@fragment
fn fs_develop(in: VertexOut) -> @location(0) vec4<f32> {
    let p = (s.geo * vec3(in.uv, 1.0)).xy;
    var c = textureSampleLevel(src, smp, select(in.uv, p, s.start == 0u), 0.0).rgb;
    for (var i = s.start; i < s.end; i++) {
        let o = s.ops[i];
        var r = c;
        switch o.kind {
            case 1u: { r = c * exp2(o.a.x); }
            case 2u: { r = max(o.m * c, vec3(0.0)); }
            case 3u: { r = 0.18 * pow(max(c, vec3(0.0)) / 0.18, vec3(o.a.x)); }
            case 4u: {
                let spread = clamp(max(c.r, max(c.g, c.b)) - min(c.r, min(c.g, c.b)), 0.0, 1.0);
                r = max(mix(vec3(dot(c, LUMA)), c, o.a.x + o.a.y * (1.0 - spread)), vec3(0.0));
            }
            case 5u: { r = dec(curve(clamp(enc(c), vec3(0.0), vec3(1.0)), o.mode)); }
            case 6u: { let e = clamp(enc(c), vec3(0.0), vec3(1.0)); r = dec(mix(e, blend(e, o.a.rgb, o.mode), o.a.w)); }
            case 7u: {
                let k = vec2(mix(1.0, s.ratio, o.a.z), 1.0);
                let d = length((in.uv - 0.5) * 2.0 * k) / length(k);
                r = c * max(1.0 + o.a.x * smoothstep(o.a.y, o.a.y + o.a.w, d), 0.0);
            }
            case 8u: { r = textureSampleLevel(blr, smp, in.uv, 0.0).rgb; }
            default: {}
        }
        c = mix(c, r, mask(o.mask, p, c) * o.opacity);
    }
    return vec4(c, 1.0);
}

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453);
}

@fragment
fn fs_output(in: VertexOut) -> @location(0) vec4<f32> {
    let c = textureSampleLevel(src, smp, in.uv, 0.0).rgb;
    let n = hash(in.position.xy) + hash(in.position.yx + 0.37) - 1.0;
    return vec4(dec(clamp(enc(c) + n / 255.0, vec3(0.0), vec3(1.0))), 1.0);
}
