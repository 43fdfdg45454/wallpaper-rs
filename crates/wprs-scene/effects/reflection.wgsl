fn reflection_turn(v: vec2<f32>, degrees: f32) -> vec2<f32> {
    let a = radians(degrees);
    return vec2<f32>(cos(a) * v.x - sin(a) * v.y, sin(a) * v.x + cos(a) * v.y);
}

@fragment
fn fs_reflection(in: VertexOut) -> @location(0) vec4<f32> {
    let pivot = vec2<f32>(0.5, param(0u).x);
    let r = reflection_turn(layer_uv(in.uv) - pivot, -param(6u).x);
    let below = r.y;
    let wave = sin(TAU * (below * param(4u).x - frame.time * param(5u).x)) * min(below * 16.0, 1.0);
    let back = pivot + reflection_turn(vec2<f32>(r.x, -below), param(6u).x);
    let mirror = from_layer(back) + px(param(3u).x) * reflection_turn(vec2<f32>(wave, 0.0), param(6u).x);
    let span = max(1.0 - param(0u).x, 1e-3);
    let k = param(1u).x * (1.0 - param(2u).x * clamp(below / span, 0.0, 1.0)) * mask_at(in.uv);
    return mix(tap(in.uv), tap(mirror), select(0.0, k, below > 0.0));
}
