fn color_tint(color: vec4<f32>, uv: vec2<f32>) -> vec4<f32> {
    let c = straight(color);
    let s = from_linear(param(0u).rgb);
    let tinted = blend_pick(u32(param(1u).x + 0.5), c.rgb, s);
    let rgb = select(tinted, blend_set_lum(s, blend_lum(c.rgb)), param(3u).x > 0.5);
    let amount = param(2u).x * param(0u).a * mask_at(uv);
    return vec4<f32>(mix(c.rgb, clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), amount) * c.a, c.a);
}
