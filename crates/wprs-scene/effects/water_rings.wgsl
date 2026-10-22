fn uv_water_rings(uv: vec2<f32>) -> vec2<f32> {
    let unit = px(1.0);
    let d = (uv - from_layer(param(4u).xy)) / unit;
    let r = length(d);
    let fade = exp(-r / param(3u).x) * min(r / param(1u).x, 1.0);
    let wave = sin(TAU * (r - frame.time * param(2u).x) / param(1u).x) * fade;
    return uv - d / max(r, 1e-3) * wave * param(0u).x * unit * mask_at(uv);
}
