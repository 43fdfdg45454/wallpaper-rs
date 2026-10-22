fn uv_twirl(uv: vec2<f32>) -> vec2<f32> {
    let unit = px(1.0);
    let c = from_layer(param(2u).xy);
    let d = (uv - c) / unit;
    let k = max(1.0 - length(d) / max(param(1u).x, 1e-3), 0.0);
    let a = radians(param(0u).x) * k * k * mask_at(uv);
    return c + mat2x2<f32>(cos(a), sin(a), -sin(a), cos(a)) * d * unit;
}
