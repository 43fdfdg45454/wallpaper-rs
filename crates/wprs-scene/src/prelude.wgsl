struct Fx {
    slot: array<vec4<f32>, 24>,
    texel: vec4<f32>,
    channel: vec4<f32>,
    mask_m0: vec4<f32>,
    mask_m1: vec4<f32>,
    info: vec4<f32>,
    area: vec4<f32>,
    clip: vec4<f32>,
    velocity: vec4<f32>,
};

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var wrap: sampler;
@group(1) @binding(0) var<uniform> fx: Fx;
@group(1) @binding(1) var input_tex: texture_2d<f32>;
@group(1) @binding(2) var mask_tex: texture_2d<f32>;
@group(1) @binding(3) var aux0: texture_2d<f32>;
@group(1) @binding(4) var aux1: texture_2d<f32>;
@group(1) @binding(5) var aux2: texture_2d<f32>;
@group(1) @binding(6) var aux3: texture_2d<f32>;
@group(1) @binding(7) var lut_tex: texture_3d<f32>;

var<private> fx_base: u32;

const TAU: f32 = 6.2831853;
const LUMA: vec3<f32> = vec3<f32>(0.2126, 0.7152, 0.0722);

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VertexOut {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return VertexOut(vec4<f32>(p.x * 2.0 - 1.0, 1.0 - p.y * 2.0, 0.0, 1.0), p);
}

fn param(i: u32) -> vec4<f32> {
    return fx.slot[fx_base + i];
}

fn px(v: f32) -> vec2<f32> {
    return v * fx.info.w * fx.texel.xy;
}

fn sample(t: texture_2d<f32>, uv: vec2<f32>) -> vec4<f32> {
    let half = 0.5 / vec2<f32>(textureDimensions(t));
    let lo = fx.clip.xy + half;
    return textureSampleLevel(t, samp, clamp(uv, lo, max(fx.clip.zw - half, lo)), 0.0);
}

fn tap(uv: vec2<f32>) -> vec4<f32> {
    return sample(input_tex, uv);
}

fn layer_uv(uv: vec2<f32>) -> vec2<f32> {
    let p = vec3<f32>(uv, 1.0);
    return vec2<f32>(dot(fx.mask_m0.xyz, p), dot(fx.mask_m1.xyz, p));
}

fn from_layer(p: vec2<f32>) -> vec2<f32> {
    let a = fx.mask_m0;
    let b = fx.mask_m1;
    let q = p - vec2<f32>(a.z, b.z);
    return vec2<f32>(b.y * q.x - a.y * q.y, a.x * q.y - b.x * q.x) / (a.x * b.y - a.y * b.x);
}

fn mask_at(uv: vec2<f32>) -> f32 {
    let m = dot(textureSampleLevel(mask_tex, samp, layer_uv(uv), 0.0), fx.channel);
    return mix(mix(m, 1.0 - m, fx.info.x), 1.0, fx.mask_m0.w);
}

fn asset_at(t: texture_2d<f32>, uv: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(t, samp, layer_uv(uv), 0.0);
}

fn cursor_uv() -> vec2<f32> {
    return (frame.cursor * frame.resolution - fx.area.xy) / max(fx.area.zw, vec2<f32>(1.0));
}

fn screen_uv(uv: vec2<f32>) -> vec2<f32> {
    return (fx.area.xy + uv * fx.area.zw) / max(frame.resolution, vec2<f32>(1.0));
}

fn band(i: u32) -> f32 {
    return frame.audio[min(i, 63u) / 4u][i % 4u];
}

fn lut(c: vec3<f32>) -> vec3<f32> {
    let n = f32(textureDimensions(lut_tex).x);
    let uvw = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)) * (n - 1.0) / n + 0.5 / n;
    return textureSampleLevel(lut_tex, samp, uvw, 0.0).rgb;
}

fn straight(c: vec4<f32>) -> vec4<f32> {
    return select(vec4<f32>(0.0), vec4<f32>(c.rgb / c.a, c.a), c.a > 1e-5);
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    let decoded = select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
    return select(c, decoded, frame.srgb > 0.5);
}

fn from_linear(c: vec3<f32>) -> vec3<f32> {
    let encoded = select(1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3<f32>(0.0031308));
    return select(c, encoded, frame.srgb > 0.5);
}

@fragment
fn fs_mask_mix(in: VertexOut) -> @location(0) vec4<f32> {
    return mix(sample(aux0, in.uv), tap(in.uv), mask_at(in.uv));
}

@fragment
fn fs_copy(in: VertexOut) -> @location(0) vec4<f32> {
    return textureSampleLevel(input_tex, samp, in.uv, 0.0);
}
