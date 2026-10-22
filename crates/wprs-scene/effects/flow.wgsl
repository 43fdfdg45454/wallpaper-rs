@fragment
fn fs_flow(in: VertexOut) -> @location(0) vec4<f32> {
    let m = asset_at(aux0, in.uv);
    let dir = select(param(3u).xy, m.rg * 2.0 - 1.0, m.a > 0.0);
    let v = dir * px(param(1u).x) * mask_at(in.uv);
    let phase = fract(frame.time * param(2u).x + vec2<f32>(0.0, 0.5));
    return mix(tap(in.uv - v * phase.x), tap(in.uv - v * phase.y), abs(1.0 - 2.0 * phase.x));
}
