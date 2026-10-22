fn posterize_srgb(c: vec3<f32>, encode: bool) -> vec3<f32> {
    let x = max(c, vec3<f32>(0.0));
    let e = select(1.055 * pow(x, vec3<f32>(1.0 / 2.4)) - 0.055, x * 12.92, x <= vec3<f32>(0.0031308));
    let d = select(pow((x + 0.055) / 1.055, vec3<f32>(2.4)), x / 12.92, x <= vec3<f32>(0.04045));
    return select(select(d, e, encode), c, frame.srgb > 0.5);
}

fn color_posterize(color: vec4<f32>, uv: vec2<f32>) -> vec4<f32> {
    let c = straight(color);
    let n = max(floor(param(0u).x), 2.0) - 1.0;
    let noise = (bayer(uv * fx.texel.zw) - 0.5) * param(1u).x;
    let steps = clamp(floor(posterize_srgb(c.rgb, true) * n + 0.5 + noise), vec3<f32>(0.0), vec3<f32>(n));
    return vec4<f32>(mix(c.rgb, posterize_srgb(steps / n, false), mask_at(uv)) * c.a, c.a);
}
