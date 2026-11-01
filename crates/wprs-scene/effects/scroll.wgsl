fn uv_scroll(uv: vec2<f32>) -> vec2<f32> {
    let l = layer_uv(uv);
    let edge = abs(layer_uv(uv + fx.texel.xy) - l) * 0.5;
    let moved = l - param(0u).xy * frame.time;
    let repeat = vec2<bool>(param(1u).x > 0.5, param(2u).x > 0.5);
    let s = from_layer(clamp(select(moved, fract(moved), repeat), edge, 1.0 - edge));
    let inside = all(l >= vec2<f32>(0.0)) && all(l <= vec2<f32>(1.0));
    let gone = any(!repeat & ((moved < vec2<f32>(0.0)) | (moved > vec2<f32>(1.0)))) && mask_at(uv) > 0.5;
    return select(select(uv, mix(uv, s, mask_at(uv)), inside), OUTSIDE_UV, gone && inside);
}
