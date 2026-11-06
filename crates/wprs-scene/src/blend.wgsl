fn blend_lum(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.3, 0.59, 0.11));
}

fn blend_set_lum(c: vec3<f32>, l: f32) -> vec3<f32> {
    let o = c + (l - blend_lum(c));
    let m = blend_lum(o);
    let n = min(o.r, min(o.g, o.b));
    let x = max(o.r, max(o.g, o.b));
    var r = o;
    if n < 0.0 {
        r = m + (r - m) * m / max(m - n, 1e-5);
    }
    if x > 1.0 {
        r = m + (r - m) * (1.0 - m) / max(x - m, 1e-5);
    }
    return r;
}

fn blend_hsl(c: vec3<f32>) -> vec3<f32> {
    let hi = max(c.r, max(c.g, c.b));
    let lo = min(c.r, min(c.g, c.b));
    let d = hi - lo;
    let l = (hi + lo) * 0.5;
    if d <= 1e-6 {
        return vec3<f32>(0.0, 0.0, l);
    }
    let s = select(d / max(2.0 - hi - lo, 1e-6), d / max(hi + lo, 1e-6), l < 0.5);
    var h = 4.0 + (c.r - c.g) / d;
    if c.r >= hi {
        h = (c.g - c.b) / d;
    } else if c.g >= hi {
        h = 2.0 + (c.b - c.r) / d;
    }
    return vec3<f32>(fract(h / 6.0), s, l);
}

fn blend_rgb(hsl: vec3<f32>) -> vec3<f32> {
    let l = hsl.z;
    if hsl.y <= 0.0 {
        return vec3<f32>(l);
    }
    let q = select(l + hsl.y - l * hsl.y, l * (1.0 + hsl.y), l < 0.5);
    let p = 2.0 * l - q;
    let t = fract(hsl.x + vec3<f32>(1.0 / 3.0, 0.0, -1.0 / 3.0));
    let rise = p + (q - p) * 6.0 * t;
    let fall = p + (q - p) * (2.0 / 3.0 - t) * 6.0;
    let late = select(vec3<f32>(p), fall, 3.0 * t < vec3<f32>(2.0));
    return select(select(late, vec3<f32>(q), 2.0 * t < vec3<f32>(1.0)), rise, 6.0 * t < vec3<f32>(1.0));
}

fn blend_hsl_mode(mode: u32, hb: vec3<f32>, hs: vec3<f32>) -> vec3<f32> {
    let pick = vec3<bool>(mode == 13u || mode == 15u, mode == 14u || mode == 15u, mode == 16u);
    return blend_rgb(select(hb, hs, pick));
}

fn blend_dodge(b: vec3<f32>, s: vec3<f32>) -> vec3<f32> {
    let d = min(vec3<f32>(1.0), b / max(1.0 - s, vec3<f32>(1e-5)));
    return select(d, vec3<f32>(1.0), s >= vec3<f32>(1.0));
}

fn blend_burn(b: vec3<f32>, s: vec3<f32>) -> vec3<f32> {
    let d = max(vec3<f32>(0.0), 1.0 - (1.0 - b) / max(s, vec3<f32>(1e-5)));
    return select(d, vec3<f32>(0.0), s <= vec3<f32>(0.0));
}

fn blend_hard(b: vec3<f32>, s: vec3<f32>) -> vec3<f32> {
    return select(1.0 - 2.0 * (1.0 - s) * (1.0 - b), 2.0 * s * b, s < vec3<f32>(0.5));
}

fn blend_soft(b: vec3<f32>, s: vec3<f32>) -> vec3<f32> {
    let dark = 2.0 * b * s + b * b * (1.0 - 2.0 * s);
    let light = sqrt(max(b, vec3<f32>(0.0))) * (2.0 * s - 1.0) + 2.0 * b * (1.0 - s);
    return select(light, dark, s < vec3<f32>(0.5));
}

fn blend_reflect(b: vec3<f32>, s: vec3<f32>) -> vec3<f32> {
    let r = min(b * b / max(1.0 - s, vec3<f32>(1e-5)), vec3<f32>(1.0));
    return select(r, vec3<f32>(1.0), s >= vec3<f32>(1.0));
}

fn blend_pick(mode: u32, b: vec3<f32>, s: vec3<f32>) -> vec3<f32> {
    let low = s < vec3<f32>(0.5);
    switch mode {
        case 0u: { return s; }
        case 1u: { return b + s; }
        case 2u: { return b * s; }
        case 3u: { return b + s - b * s; }
        case 4u: { return max(b, s); }
        case 5u: { return min(b, s); }
        case 6u: { return blend_hard(s, b); }
        case 7u: { return blend_soft(b, s); }
        case 8u: { return blend_hard(b, s); }
        case 9u: { return abs(b - s); }
        case 10u: { return b + s - 2.0 * b * s; }
        case 11u: { return blend_dodge(b, s); }
        case 12u: { return blend_burn(b, s); }
        case 13u, 14u, 15u, 16u: { return blend_hsl_mode(mode, blend_hsl(b), blend_hsl(s)); }
        case 17u: { return max(b + s - 1.0, vec3<f32>(0.0)); }
        case 18u: { return min(b + s, vec3<f32>(1.0)); }
        case 19u: { return select(b, s, blend_lum(s) < blend_lum(b)); }
        case 20u: { return select(b, s, blend_lum(s) > blend_lum(b)); }
        case 21u: { return select(blend_dodge(b, 2.0 * s - 1.0), blend_burn(b, 2.0 * s), low); }
        case 22u: { return clamp(b + 2.0 * s - 1.0, vec3<f32>(0.0), vec3<f32>(1.0)); }
        case 23u: { return select(max(b, 2.0 * s - 1.0), min(b, 2.0 * s), low); }
        case 24u: { return select(vec3<f32>(0.0), vec3<f32>(1.0), b + s >= vec3<f32>(1.0)); }
        case 25u: { return max(b - s, vec3<f32>(0.0)); }
        case 26u: { return select(min(b / max(s, vec3<f32>(1e-5)), vec3<f32>(1.0)), vec3<f32>(1.0), s <= vec3<f32>(0.0)); }
        case 27u: { return blend_reflect(b, s); }
        case 28u: { return blend_reflect(s, b); }
        case 29u: { return min(b, s) - max(b, s) + 1.0; }
        case 30u: { return (b + s) * 0.5; }
        case 31u: { return 1.0 - abs(1.0 - b - s); }
        case 32u: { return max(b.r, max(b.g, b.b)) * s; }
        default: { return b + b * s; }
    }
}

fn blend_mode(mode: u32, b: vec3<f32>, s: vec3<f32>, amount: f32) -> vec3<f32> {
    return mix(b, blend_pick(mode, b, s), amount);
}
