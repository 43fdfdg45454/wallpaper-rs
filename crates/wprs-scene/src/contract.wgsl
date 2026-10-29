struct More {
    daytime: f32,
    layer_alpha: f32,
    parallax: vec2<f32>,
    layer_color: vec4<f32>,
    sizes: array<vec4<f32>, 4>,
    bands_l: array<vec4<f32>, 16>,
    bands_r: array<vec4<f32>, 16>,
};

@group(1) @binding(8) var aux4: texture_2d<f32>;
@group(1) @binding(9) var aux5: texture_2d<f32>;
@group(1) @binding(10) var aux6: texture_2d<f32>;
@group(1) @binding(11) var aux7: texture_2d<f32>;
@group(1) @binding(12) var<uniform> more: More;

const PI: f32 = 3.14159265;

fn daytime() -> f32 {
    return more.daytime;
}

fn parallax() -> vec2<f32> {
    return more.parallax;
}

fn layer_color() -> vec4<f32> {
    return more.layer_color;
}

fn layer_alpha() -> f32 {
    return more.layer_alpha;
}

fn tex_size(i: u32) -> vec2<f32> {
    let j = min(i, 7u);
    let s = more.sizes[j / 2u];
    return select(s.xy, s.zw, (j & 1u) == 1u);
}

fn band_left(i: u32) -> f32 {
    let j = min(i, 63u);
    return more.bands_l[j / 4u][j % 4u];
}

fn band_right(i: u32) -> f32 {
    let j = min(i, 63u);
    return more.bands_r[j / 4u][j % 4u];
}

fn band_of(i: u32, count: u32) -> f32 {
    let w = 64u / clamp(count, 1u, 64u);
    var sum = 0.0;
    for (var k = 0u; k < w; k = k + 1u) {
        sum = sum + band(min(i * w + k, 63u));
    }
    return sum / f32(w);
}

fn rotate2(v: vec2<f32>, degrees: f32) -> vec2<f32> {
    let a = radians(degrees);
    return vec2<f32>(cos(a) * v.x - sin(a) * v.y, sin(a) * v.x + cos(a) * v.y);
}

fn greyscale(c: vec3<f32>) -> f32 {
    return dot(c, LUMA);
}

fn hsv(c: vec3<f32>) -> vec3<f32> {
    let hi = max(c.r, max(c.g, c.b));
    let d = hi - min(c.r, min(c.g, c.b));
    var h = 0.0;
    if d > 1e-5 {
        if hi == c.r {
            h = (c.g - c.b) / d;
        } else if hi == c.g {
            h = (c.b - c.r) / d + 2.0;
        } else {
            h = (c.r - c.g) / d + 4.0;
        }
    }
    return vec3<f32>(fract(h / 6.0 + 1.0), select(0.0, d / hi, hi > 1e-5), hi);
}

fn from_hsv(c: vec3<f32>) -> vec3<f32> {
    let p = (vec3<f32>(5.0, 3.0, 1.0) + fract(c.x) * 6.0) % vec3<f32>(6.0);
    return c.z - c.z * c.y * clamp(min(p, 4.0 - p), vec3<f32>(0.0), vec3<f32>(1.0));
}
