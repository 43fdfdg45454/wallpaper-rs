struct Fx {
    slot: array<vec4<f32>, 16>,
    texel: vec4<f32>,
    channel: vec4<f32>,
    mask_m0: vec4<f32>,
    mask_m1: vec4<f32>,
    info: vec4<f32>,
};

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var samp: sampler;
@group(1) @binding(0) var<uniform> fx: Fx;
@group(1) @binding(1) var input_tex: texture_2d<f32>;
@group(1) @binding(2) var mask_tex: texture_2d<f32>;
@group(1) @binding(3) var aux0: texture_2d<f32>;
@group(1) @binding(4) var aux1: texture_2d<f32>;
@group(1) @binding(5) var aux2: texture_2d<f32>;
@group(1) @binding(6) var aux3: texture_2d<f32>;

const TAU: f32 = 6.2831853;
const LUMA: vec3<f32> = vec3<f32>(0.2126, 0.7152, 0.0722);

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VertexOut {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return VertexOut(vec4<f32>(p.x * 2.0 - 1.0, 1.0 - p.y * 2.0, 0.0, 1.0), p);
}

fn param(i: u32) -> vec4<f32> {
    return fx.slot[i];
}

fn px(v: f32) -> vec2<f32> {
    return v * fx.info.w * fx.texel.xy;
}

fn sample(t: texture_2d<f32>, uv: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(t, samp, uv, 0.0);
}

fn tap(uv: vec2<f32>) -> vec4<f32> {
    return sample(input_tex, uv);
}

fn mask_at(uv: vec2<f32>) -> f32 {
    let p = vec3<f32>(uv, 1.0);
    let m = dot(sample(mask_tex, vec2<f32>(dot(fx.mask_m0.xyz, p), dot(fx.mask_m1.xyz, p))), fx.channel);
    return mix(m, 1.0 - m, fx.info.x);
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
