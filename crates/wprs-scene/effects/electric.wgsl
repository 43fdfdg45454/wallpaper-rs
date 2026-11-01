fn color_electric(color: vec4<f32>, uv: vec2<f32>) -> vec4<f32> {
    let l = layer_uv(uv);
    let span = layer_extent();
    let p = vec2<f32>(l.x * span.x / max(span.y, 1.0), l.y) * param(3u).x * 4.0 + frame.seed;
    let rate = select(1.0, 0.5, frame.quality < 0.5);
    let t = floor(frame.time * 30.0 * rate) / (30.0 * rate) * param(5u).x;
    let a = fbm(p + vec2<f32>(t * 0.7, -t * 0.4), 3);
    let b = fbm(p * 1.3 + vec2<f32>(-t * 0.5, t * 0.8) + 7.7, 3);
    let near = 1.0 - min(abs(a - b) * 4.0, 1.0);
    let sharp = pow(near, mix(24.0, 2.0, param(4u).x));
    let line = smoothstep(param(2u).x, max(param(2u).y, param(2u).x + 1e-3), sharp);
    let tone = mix(from_linear(param(0u).rgb), from_linear(param(1u).rgb), line) * param(6u).x;
    let c = straight(color);
    let mode = u32(param(7u).x + 0.5);
    let k = line * mix(param(0u).a, param(1u).a, line) * mask_at(uv);
    let rgb = clamp(blend_mode(mode, c.rgb, tone, k), vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(rgb * c.a, c.a);
}
