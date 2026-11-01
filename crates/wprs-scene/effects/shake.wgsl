fn uv_shake(uv: vec2<f32>) -> vec2<f32> {
    let t = frame.time * param(1u).x + time_phase(aux0, uv) * 8.0;
    let calm = vec2<f32>(noise1(t), noise1(t + 17.3)) * 2.0 - 1.0;
    let jitter = vec2<f32>(noise1(t * 7.0 + 3.1), noise1(t * 7.0 + 41.9)) * 2.0 - 1.0;
    let n = mix(calm, jitter, param(3u).x);
    let held = sign(n) * pow(abs(n), vec2<f32>(1.0 + 3.0 * param(4u).x));
    let kept = clamp(held, -param(5u).xy, param(5u).xy);
    return uv + kept * param(2u).xy * px(param(0u).x) * mask_at(uv);
}
