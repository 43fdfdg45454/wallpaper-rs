fn uv_twirl(uv: vec2<f32>) -> vec2<f32> {
    let unit = px(1.0);
    let c = from_layer(param(2u).xy);
    let d = (uv - c) / unit;
    let r = param(1u).x;
    let reach = length(d * vec2<f32>(1.0, param(3u).x)) / max(r, 1e-3);
    let k = clamp((1.0 - reach) / max(1.0 - param(4u).x, 1e-3), 0.0, 1.0);
    let wobble = 1.0 + param(5u).x * gradient_noise(d / max(r, 1e-3) * 4.0 + frame.seed);
    let a = radians(param(0u).x) * k * k * wobble * mask_at(uv);
    let turned = c + mat2x2<f32>(cos(a), sin(a), -sin(a), cos(a)) * d * unit;
    return select(turned, from_layer(fract(layer_uv(turned))), param(6u).x > 0.5);
}
