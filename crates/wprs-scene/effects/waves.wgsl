fn uv_waves(uv: vec2<f32>) -> vec2<f32> {
    let angle = radians(param(3u).x);
    let dir = vec2<f32>(cos(angle), sin(angle));
    let phase = (dot(uv, dir) * param(1u).x - frame.time * param(2u).x) * TAU;
    return uv + vec2<f32>(-dir.y, dir.x) * sin(phase) * px(param(0u).x) * mask_at(uv);
}
