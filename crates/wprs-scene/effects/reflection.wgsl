@fragment
fn fs_reflection(in: VertexOut) -> @location(0) vec4<f32> {
    let l = layer_uv(in.uv);
    let below = l.y - param(0u).x;
    let wave = sin(TAU * (below * param(4u).x - frame.time * param(5u).x)) * min(below * 16.0, 1.0);
    let mirror = from_layer(vec2<f32>(l.x, param(0u).x - below)) + px(param(3u).x) * vec2<f32>(wave, 0.0);
    let span = max(1.0 - param(0u).x, 1e-3);
    let k = param(1u).x * (1.0 - param(2u).x * clamp(below / span, 0.0, 1.0)) * mask_at(in.uv);
    return mix(tap(in.uv), tap(mirror), select(0.0, k, below > 0.0));
}
