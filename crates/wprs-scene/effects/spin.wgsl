fn uv_spin(uv: vec2<f32>) -> vec2<f32> {
    let unit = px(1.0);
    let c = from_layer(param(0u).xy);
    let d = (uv - c) / unit;
    let span = layer_extent();
    let r = length(d) / max(max(span.x, span.y), 1.0);
    let edge = max(param(3u).x, 1e-3);
    let k = 1.0 - smoothstep(edge * 0.75, edge, r);
    let a = -radians(param(1u).x + param(2u).x * frame.time) * k * mask_at(uv);
    return c + mat2x2<f32>(cos(a), sin(a), -sin(a), cos(a)) * d * unit;
}
