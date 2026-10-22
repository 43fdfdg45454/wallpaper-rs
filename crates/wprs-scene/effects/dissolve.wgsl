fn color_dissolve(color: vec4<f32>, uv: vec2<f32>) -> vec4<f32> {
    let n = clamp(fbm((uv - from_layer(vec2<f32>(0.0))) / px(param(1u).x), 4) * 0.7 + 0.5, 0.0, 0.999);
    let e = param(2u).x;
    let t = mix(-e, 1.0, param(0u).x);
    let rim = 1.0 - smoothstep(t, t + e, n);
    let glow = from_linear(param(3u).rgb) * param(3u).a * rim * color.a;
    return vec4<f32>(min(color.rgb + glow, vec3<f32>(color.a)), color.a) * step(t, n);
}
