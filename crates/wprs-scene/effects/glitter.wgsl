fn color_glitter(color: vec4<f32>, uv: vec2<f32>) -> vec4<f32> {
    let size = param(1u).x * select(1.0, 2.0, frame.quality < 0.5);
    let cell = px(size * 6.0);
    let p = (uv - from_layer(vec2<f32>(0.0))) / cell;
    let id = floor(p);
    let shot = floor(frame.time * param(2u).x) % 512.0;
    let seed = id + shot * vec2<f32>(13.1, 71.7) + frame.seed;
    let on = step(hash(seed), param(0u).x);
    let at = (fract(p) - 0.2 - 0.6 * hash2(seed + 5.3)) * 6.0;
    let glow = exp(-4.0 * dot(at, at));
    let cross = max(0.0, 1.0 - abs(at.x * at.y) * 6.0) * max(0.0, 1.0 - length(at) / 3.0);
    let twinkle = 0.5 + 0.5 * hash(seed + 9.1);
    let light = (glow + cross) * twinkle * on * mask_at(uv);
    let tone = from_linear(param(3u).rgb) * param(3u).a;
    return vec4<f32>(min(color.rgb + tone * light * color.a, vec3<f32>(color.a)), color.a);
}
