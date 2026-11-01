fn waves_one(uv: vec2<f32>, degrees: f32, depth: f32, shift: f32) -> vec2<f32> {
    let angle = radians(degrees);
    let dir = vec2<f32>(cos(angle), sin(angle));
    let phase = (dot(uv, dir) * param(1u).x / depth - frame.time * param(2u).x) * TAU + shift;
    return vec2<f32>(-dir.y, dir.x) * sin(phase) * px(param(0u).x) * depth;
}

fn uv_waves(uv: vec2<f32>) -> vec2<f32> {
    let depth = mix(1.0, 0.25 + 0.75 * clamp(layer_uv(uv).y, 0.0, 1.0), param(6u).x);
    let shift = time_phase(aux0, uv) * TAU;
    var moved = waves_one(uv, param(3u).x, depth, shift);
    if param(4u).x > 1.5 {
        moved += waves_one(uv, param(5u).x, depth, shift);
    }
    return uv + moved * mask_at(uv);
}
