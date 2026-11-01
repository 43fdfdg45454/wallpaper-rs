fn flow_field(uv: vec2<f32>) -> vec2<f32> {
    let p = layer_uv(uv) * 3.0 + frame.seed;
    let turn = TAU * value_noise(p) + 2.0 * value_noise(p * 2.7 + 11.0);
    return vec2<f32>(cos(turn), sin(turn)) * length(param(3u).xy);
}

@fragment
fn fs_flow(in: VertexOut) -> @location(0) vec4<f32> {
    let m = asset_at(aux0, in.uv);
    let fixed = select(param(3u).xy, flow_field(in.uv), param(4u).x > 0.5);
    let dir = select(fixed, m.rg * 2.0 - 1.0, m.a > 0.0);
    let v = dir * px(param(1u).x) * mask_at(in.uv);
    let phase = fract(frame.time * param(2u).x + time_phase(aux1, in.uv) + vec2<f32>(0.0, 0.5));
    return mix(tap(in.uv - v * phase.x), tap(in.uv - v * phase.y), abs(1.0 - 2.0 * phase.x));
}
