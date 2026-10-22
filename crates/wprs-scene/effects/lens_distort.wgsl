fn uv_lens_distort(uv: vec2<f32>) -> vec2<f32> {
    let unit = px(1.0);
    let o = from_layer(vec2<f32>(0.0));
    let w = (from_layer(vec2<f32>(1.0, 0.0)) - o) / unit;
    let h = (from_layer(vec2<f32>(0.0, 1.0)) - o) / unit;
    let half = 0.5 * sqrt(dot(w, w) + dot(h, h));
    let c = from_layer(param(3u).xy);
    let d = (uv - c) / unit / half;
    let r2 = dot(d, d);
    let k = vec2<f32>(param(0u).x, param(1u).x);
    let fit = select(1.0, max(1.0 + k.x + k.y, 0.1), param(2u).x > 0.5);
    let warped = c + d * (1.0 + k.x * r2 + k.y * r2 * r2) / fit * half * unit;
    return mix(uv, warped, mask_at(uv));
}
