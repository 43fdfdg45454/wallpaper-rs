struct Item {
    m0: vec4<f32>,
    m1: vec4<f32>,
    quad: vec4<f32>,
    color: vec4<f32>,
    uv: vec4<f32>,
    size: vec4<f32>,
    shape: vec4<f32>,
    place: vec4<f32>,
    stroke: vec4<f32>,
    style: vec4<f32>,
    grad: vec4<f32>,
    stops: vec4<f32>,
    stop_colors: array<vec4<f32>, 4>,
    channel: vec4<f32>,
    mask_info: vec4<f32>,
    mask_m0: vec4<f32>,
    mask_m1: vec4<f32>,
    mask_shape: vec4<f32>,
    back: vec4<f32>,
    mode: vec4<f32>,
};

struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> item: Item;
@group(1) @binding(0) var src_tex: texture_2d<f32>;
@group(1) @binding(1) var mask_tex: texture_2d<f32>;
@group(1) @binding(2) var back_tex: texture_2d<f32>;

const PI: f32 = 3.14159265;

@vertex
fn vs_item(@builtin(vertex_index) i: u32) -> Out {
    let corner = vec2<f32>(f32(i & 1u), f32((i >> 1u) & 1u));
    let u = mix(item.quad.xy, item.quad.zw, corner);
    let p = vec3<f32>(u, 1.0);
    return Out(vec4<f32>(dot(item.m0.xyz, p), dot(item.m1.xyz, p), 0.0, 1.0), u);
}

fn encode(c: vec3<f32>) -> vec3<f32> {
    return select(1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3<f32>(0.0031308));
}

fn decode(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
}

fn sdf(kind: f32, p: vec2<f32>, size: vec2<f32>, a: vec3<f32>) -> f32 {
    let h = size * 0.5;
    if kind < 0.5 {
        let q = abs(p) - h + a.x;
        return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - a.x;
    }
    let ellipse = (length(p / max(h, vec2<f32>(1e-3))) - 1.0) * min(h.x, h.y);
    if kind < 1.5 {
        return ellipse;
    }
    if kind < 2.5 {
        return abs(ellipse + a.x * 0.5) - a.x * 0.5;
    }
    let r = min(h.x, h.y);
    let sector = PI / max(a.x, 2.0);
    let angle = atan2(p.x, -p.y);
    let local = abs((angle % (2.0 * sector) + 2.0 * sector) % (2.0 * sector) - sector);
    if kind < 3.5 {
        return length(p) * cos(local) - r * cos(sector);
    }
    if kind < 4.5 {
        let edge = mix(r * a.y, r, local / sector);
        return (length(p) - edge) * 0.5;
    }
    let b = a.yz;
    let t = clamp(dot(p, b) / max(dot(b, b), 1e-6), 0.0, 1.0);
    return length(p - b * t) - a.x * 0.5;
}

fn coverage(d: f32) -> f32 {
    return clamp(0.5 - d / max(fwidth(d), 1e-4), 0.0, 1.0);
}

fn mask_value(in: Out) -> f32 {
    let kind = item.mask_info.x;
    if kind < 0.5 {
        return 1.0;
    }
    let p = vec3<f32>(in.position.xy, 1.0);
    let at = vec2<f32>(dot(item.mask_m0.xyz, p), dot(item.mask_m1.xyz, p));
    var m: f32;
    if kind < 2.5 {
        m = dot(textureSampleLevel(mask_tex, samp, at, 0.0), item.channel);
    } else {
        let d = sdf(item.mask_shape.x, at - item.mask_shape.zw * 0.5, item.mask_shape.zw, vec3<f32>(item.mask_shape.y, 0.5, 0.0));
        m = 1.0 - smoothstep(-item.mask_info.z, item.mask_info.z + 1.0, d);
    }
    return mix(m, 1.0 - m, item.mask_info.y);
}

fn lum(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.3, 0.59, 0.11));
}

fn clip_color(c: vec3<f32>) -> vec3<f32> {
    let l = lum(c);
    let n = min(c.r, min(c.g, c.b));
    let x = max(c.r, max(c.g, c.b));
    var o = c;
    if n < 0.0 {
        o = l + (o - l) * l / max(l - n, 1e-5);
    }
    if x > 1.0 {
        o = l + (o - l) * (1.0 - l) / max(x - l, 1e-5);
    }
    return o;
}

fn set_lum(c: vec3<f32>, l: f32) -> vec3<f32> {
    return clip_color(c + (l - lum(c)));
}

fn sat(c: vec3<f32>) -> f32 {
    return max(c.r, max(c.g, c.b)) - min(c.r, min(c.g, c.b));
}

fn set_sat(c: vec3<f32>, s: f32) -> vec3<f32> {
    let lo = min(c.r, min(c.g, c.b));
    let range = sat(c);
    return select(vec3<f32>(0.0), (c - lo) * s / range, range > 1e-5);
}

fn soft_light(b: vec3<f32>, s: vec3<f32>) -> vec3<f32> {
    let d = select(sqrt(b), ((16.0 * b - 12.0) * b + 4.0) * b, b <= vec3<f32>(0.25));
    return select(b + (2.0 * s - 1.0) * (d - b), b - (1.0 - 2.0 * s) * b * (1.0 - b), s <= vec3<f32>(0.5));
}

fn hard_light(b: vec3<f32>, s: vec3<f32>) -> vec3<f32> {
    return select(1.0 - 2.0 * (1.0 - s) * (1.0 - b), 2.0 * s * b, s <= vec3<f32>(0.5));
}

fn mixed(mode: u32, b: vec3<f32>, s: vec3<f32>) -> vec3<f32> {
    switch mode {
        case 4u: { return max(b, s); }
        case 5u: { return min(b, s); }
        case 6u: { return hard_light(s, b); }
        case 7u: { return soft_light(b, s); }
        case 8u: { return hard_light(b, s); }
        case 9u: { return abs(b - s); }
        case 10u: { return b + s - 2.0 * b * s; }
        case 11u: {
            let dodge = min(vec3<f32>(1.0), b / max(1.0 - s, vec3<f32>(1e-5)));
            return select(dodge, vec3<f32>(1.0), (s >= vec3<f32>(1.0)) & (b > vec3<f32>(0.0)));
        }
        case 12u: {
            let burn = 1.0 - min(vec3<f32>(1.0), (1.0 - b) / max(s, vec3<f32>(1e-5)));
            return select(burn, vec3<f32>(0.0), (s <= vec3<f32>(0.0)) & (b < vec3<f32>(1.0)));
        }
        case 13u: { return set_lum(set_sat(s, sat(b)), lum(b)); }
        case 14u: { return set_lum(set_sat(b, sat(s)), lum(b)); }
        case 15u: { return set_lum(s, lum(b)); }
        default: { return set_lum(b, lum(s)); }
    }
}

fn finish(src: vec4<f32>, in: Out) -> vec4<f32> {
    let c = src * mask_value(in);
    if item.mode.x < 3.5 {
        return c;
    }
    let bd = textureSampleLevel(back_tex, samp, item.back.xy + in.position.xy * item.back.zw, 0.0);
    let cb = select(vec3<f32>(0.0), bd.rgb / bd.a, bd.a > 1e-5);
    let cs = select(vec3<f32>(0.0), c.rgb / c.a, c.a > 1e-5);
    let blended = mixed(u32(item.mode.x + 0.5), cb, cs);
    let rgb = (1.0 - bd.a) * c.rgb + (1.0 - c.a) * bd.rgb + c.a * bd.a * blended;
    return vec4<f32>(rgb, c.a + bd.a - c.a * bd.a);
}

@fragment
fn fs_texture(in: Out) -> @location(0) vec4<f32> {
    var t = textureSample(src_tex, samp, item.uv.xy + in.uv * item.uv.zw);
    if item.mode.y > 0.5 {
        t = vec4<f32>(decode(t.rgb), t.a);
    }
    if item.size.w > 0.5 {
        t = vec4<f32>(encode(t.rgb), t.a);
    }
    if item.size.z > 0.5 {
        t = vec4<f32>(t.rgb * t.a, t.a);
    }
    return finish(t * item.color, in);
}

@fragment
fn fs_solid(in: Out) -> @location(0) vec4<f32> {
    return finish(item.color, in);
}

fn paint(uv: vec2<f32>) -> vec4<f32> {
    if item.style.z < 0.5 {
        return item.color;
    }
    var f = clamp(dot(uv - item.grad.xy, item.grad.zw - item.grad.xy) / max(dot(item.grad.zw - item.grad.xy, item.grad.zw - item.grad.xy), 1e-6), 0.0, 1.0);
    if item.style.z > 1.5 {
        f = clamp(length(uv - item.grad.xy) / max(length(item.grad.zw - item.grad.xy), 1e-6), 0.0, 1.0);
    }
    var c = item.stop_colors[0];
    for (var i = 1u; i < u32(item.style.w); i++) {
        let a = item.stops[i - 1u];
        let k = clamp((f - a) / max(item.stops[i] - a, 1e-6), 0.0, 1.0);
        c = select(c, mix(item.stop_colors[i - 1u], item.stop_colors[i], k), f >= a);
    }
    return c * item.color.a;
}

@fragment
fn fs_shape(in: Out) -> @location(0) vec4<f32> {
    let p = in.uv * item.size.xy - item.place.xy;
    let d = sdf(item.shape.x, p, item.size.xy, item.shape.yzw);
    let fill = paint(in.uv) * coverage(d) * item.style.y;
    let w = item.style.x;
    let edge = select(0.0, coverage(abs(d) - w * 0.5), w > 0.0);
    return finish(fill * (1.0 - edge) + item.stroke * edge, in);
}

fn band(i: f32) -> f32 {
    let n = clamp(i * 64.0 / item.mode.z, 0.0, 63.0);
    let v = frame.audio[u32(n) / 4u][u32(n) % 4u];
    return clamp(v, 0.0, 1.0);
}

@fragment
fn fs_spectrum(in: Out) -> @location(0) vec4<f32> {
    let n = item.mode.z;
    var uv = in.uv;
    if item.mode.y > 0.5 && item.mode.y < 1.5 {
        let p = in.uv - 0.5;
        uv = vec2<f32>(atan2(p.x, -p.y) / (2.0 * PI) + 0.5, 1.0 - (length(p) * 2.0 - 0.4) / 0.6);
    }
    let i = floor(uv.x * n);
    let v = band(i);
    var d: f32;
    if item.mode.y > 1.5 {
        let x = uv.x * n - 0.5;
        let level = mix(band(floor(x)), band(floor(x) + 1.0), fract(x));
        d = (abs(uv.y - (1.0 - level * 0.9 - 0.05)) - 0.02) * item.size.y;
    } else {
        let gap = item.mode.w * 0.5;
        let fx = abs(fract(uv.x * n) - 0.5) - (0.5 - gap);
        d = max(fx * item.size.x / n, ((1.0 - v) - uv.y) * item.size.y);
        d = max(d, max(-uv.y, uv.y - 1.0) * item.size.y);
    }
    return finish(item.color * coverage(d), in);
}
