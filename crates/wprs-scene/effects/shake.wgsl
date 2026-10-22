fn uv_shake(uv: vec2<f32>) -> vec2<f32> {
    let t = frame.time * param(1u).x;
    let n = vec2<f32>(noise1(t), noise1(t + 17.3)) * 2.0 - 1.0;
    return uv + n * param(2u).xy * px(param(0u).x) * mask_at(uv);
}
