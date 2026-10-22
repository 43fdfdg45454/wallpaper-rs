fn color_film_grain(color: vec4<f32>, uv: vec2<f32>) -> vec4<f32> {
    let c = straight(color);
    let shot = floor(frame.time * param(2u).x) % 256.0;
    let cell = floor(uv / px(param(1u).x)) + shot * vec2<f32>(37.0, 17.0) + frame.seed;
    let grain = (hash(cell) + hash(cell + 91.7) - 1.0) * param(0u).x * mask_at(uv);
    let lab = oklab(to_linear(c.rgb));
    let l = lab.x + 2.0 * clamp(min(lab.x, 1.0 - lab.x), 0.0, 0.5) * grain;
    let rgb = from_linear(max(from_oklab(vec3<f32>(l, lab.yz)), vec3<f32>(0.0)));
    return vec4<f32>(rgb * c.a, c.a);
}
