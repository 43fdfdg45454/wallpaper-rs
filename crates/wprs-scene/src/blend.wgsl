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

fn blend_sat(c: vec3<f32>) -> f32 {
    return max(c.r, max(c.g, c.b)) - min(c.r, min(c.g, c.b));
}

fn blend_set_sat(c: vec3<f32>, s: f32) -> vec3<f32> {
    let range = blend_sat(c);
    return select(vec3<f32>(0.0), (c - min(c.r, min(c.g, c.b))) * s / range, range > 1e-5);
}

fn blend_dodge(b: vec3<f32>, s: vec3<f32>) -> vec3<f32> {
    let d = min(vec3<f32>(1.0), b / max(1.0 - s, vec3<f32>(1e-5)));
    return select(d, vec3<f32>(1.0), (s >= vec3<f32>(1.0)) & (b > vec3<f32>(0.0)));
}

fn blend_burn(b: vec3<f32>, s: vec3<f32>) -> vec3<f32> {
    let d = 1.0 - min(vec3<f32>(1.0), (1.0 - b) / max(s, vec3<f32>(1e-5)));
    return select(d, vec3<f32>(0.0), (s <= vec3<f32>(0.0)) & (b < vec3<f32>(1.0)));
}

fn blend_hard(b: vec3<f32>, s: vec3<f32>) -> vec3<f32> {
    return select(1.0 - 2.0 * (1.0 - s) * (1.0 - b), 2.0 * s * b, s <= vec3<f32>(0.5));
}

fn blend_soft(b: vec3<f32>, s: vec3<f32>) -> vec3<f32> {
    let d = select(sqrt(b), ((16.0 * b - 12.0) * b + 4.0) * b, b <= vec3<f32>(0.25));
    return select(b + (2.0 * s - 1.0) * (d - b), b - (1.0 - 2.0 * s) * b * (1.0 - b), s <= vec3<f32>(0.5));
}

fn blend_reflect(b: vec3<f32>, s: vec3<f32>) -> vec3<f32> {
    let r = min(b * b / max(1.0 - s, vec3<f32>(1e-5)), vec3<f32>(1.0));
    return select(r, vec3<f32>(1.0), s >= vec3<f32>(1.0));
}

fn blend_pick(mode: u32, b: vec3<f32>, s: vec3<f32>) -> vec3<f32> {
    let low = s <= vec3<f32>(0.5);
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
        case 13u: { return blend_set_lum(blend_set_sat(s, blend_sat(b)), blend_lum(b)); }
        case 14u: { return blend_set_lum(blend_set_sat(b, blend_sat(s)), blend_lum(b)); }
        case 15u: { return blend_set_lum(s, blend_lum(b)); }
        case 16u: { return blend_set_lum(b, blend_lum(s)); }
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
        case 32u: { return min(b * s / max(blend_lum(s), 1e-3), vec3<f32>(1.0)); }
        default: { return min(2.0 * b * s, vec3<f32>(1.0)); }
    }
}

fn blend_mode(mode: u32, b: vec3<f32>, s: vec3<f32>, amount: f32) -> vec3<f32> {
    return mix(b, blend_pick(mode, b, s), amount);
}
