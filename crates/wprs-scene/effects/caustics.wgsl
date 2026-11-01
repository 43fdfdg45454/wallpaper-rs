fn caustics_web(p: vec2<f32>, t: f32) -> f32 {
    let bend = vec2<f32>(gradient_noise(p * 0.6 + vec2<f32>(t * 0.3, 1.7)), gradient_noise(p * 0.6 - vec2<f32>(4.1, t * 0.25)));
    let n = gradient_noise(p + bend * 0.8 + vec2<f32>(0.0, t * 0.2));
    return pow(1.0 - min(abs(n) * 2.2, 1.0), 6.0);
}

fn color_caustics(color: vec4<f32>, uv: vec2<f32>) -> vec4<f32> {
    let l = layer_uv(uv);
    let span = layer_extent();
    let aspect = vec2<f32>(span.x / max(span.y, 1.0), 1.0);
    let deep = mix(1.0, 0.35 + 0.65 * clamp(l.y, 0.0, 1.0), param(4u).x);
    let p = vec2<f32>(l.x * aspect.x / deep, l.y / deep) * param(1u).x * 3.0 + frame.seed;
    let t = frame.time * param(2u).x;
    var web = caustics_web(p, t);
    if frame.quality > 0.5 {
        web = max(web, caustics_web(p * 1.7 + 3.3, t * 1.3) * 0.7);
    }
    let light = from_linear(param(0u).rgb) * param(0u).a * web * param(3u).x * mask_at(uv);
    return vec4<f32>(min(color.rgb + light * color.a, vec3<f32>(color.a)), color.a);
}
