fn uv_scroll(uv: vec2<f32>) -> vec2<f32> {
    let l = layer_uv(uv);
    let edge = abs(layer_uv(uv + fx.texel.xy) - l) * 0.5;
    let s = from_layer(clamp(fract(l - param(0u).xy * frame.time), edge, 1.0 - edge));
    let inside = all(l >= vec2<f32>(0.0)) && all(l <= vec2<f32>(1.0));
    return select(uv, mix(uv, s, mask_at(uv)), inside);
}
