fn color_film_grain(color: vec4<f32>, uv: vec2<f32>) -> vec4<f32> {
    let c = straight(color);
    let shot = floor(frame.time * param(2u).x) % 256.0;
    let cell = floor(uv / px(param(1u).x)) + shot * vec2<f32>(37.0, 17.0) + frame.seed;
    let noise = hash(cell) + hash(cell + 91.7) - 1.0;
    let grain = noise * param(0u).x * mask_at(uv);
    var lab = oklab(to_linear(c.rgb));
    lab = vec3<f32>(lab.x, lab.yz * (1.0 - param(3u).x));
    let weight = 2.0 * clamp(min(lab.x, 1.0 - lab.x), 0.0, 0.5) * (1.0 + param(4u).x * (2.0 * lab.x - 1.0));
    let mode = u32(param(5u).x + 0.5);
    var rgb = from_linear(max(from_oklab(vec3<f32>(lab.x + weight * grain, lab.yz)), vec3<f32>(0.0)));
    if mode != 6u {
        let plain = from_linear(max(from_oklab(lab), vec3<f32>(0.0)));
        let s = vec3<f32>(clamp(0.5 + noise * 0.5, 0.0, 1.0));
        rgb = clamp(blend_mode(mode, plain, s, param(0u).x * weight * mask_at(uv)), vec3<f32>(0.0), vec3<f32>(1.0));
    }
    return vec4<f32>(rgb * c.a, c.a);
}
