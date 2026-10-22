fn color_clouds(color: vec4<f32>, uv: vec2<f32>) -> vec4<f32> {
    let p = ((uv - from_layer(vec2<f32>(0.0))) / px(1.0) - param(6u).xy * frame.time) / param(5u).x;
    let drift = vec2<f32>(0.0, frame.time * 0.02);
    let q = vec2<f32>(fbm(p + drift, 2), fbm(p - drift + vec2<f32>(5.2, 1.3), 2));
    let n = fbm(p + param(7u).x * q, i32(param(8u).x)) * 0.5 + 0.5;
    let edge = 1.0 - param(3u).x;
    let density = smoothstep(edge - param(4u).x * 0.5, edge + param(4u).x * 0.5, n);
    let top = 1.0 - smoothstep(param(9u).x, param(9u).x + 0.2, layer_uv(uv).y);
    let a = density * top * param(2u).x;
    let tint = from_linear(mix(param(1u).rgb, param(0u).rgb, smoothstep(edge, edge + 0.4, n)));
    return vec4<f32>(tint * a, a) + color * (1.0 - a);
}
