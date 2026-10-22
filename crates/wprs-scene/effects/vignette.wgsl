fn color_vignette(color: vec4<f32>, uv: vec2<f32>) -> vec4<f32> {
    let wide = length(vec2<f32>(fx.mask_m1.y * fx.area.z, fx.mask_m1.x * fx.area.w));
    let tall = length(vec2<f32>(fx.mask_m0.y * fx.area.z, fx.mask_m0.x * fx.area.w));
    let aspect = vec2<f32>(mix(1.0, wide / max(tall, 1e-4), param(3u).x), 1.0);
    let r = length((layer_uv(uv) - param(4u).xy) * 2.0 * aspect) / length(aspect);
    let edge = smoothstep(param(1u).x * (1.0 - param(2u).x), param(1u).x + 1e-4, r);
    let tone = vec4<f32>(from_linear(param(5u).rgb), 1.0) * color.a;
    return mix(color, tone, edge * param(0u).x * mask_at(uv));
}
