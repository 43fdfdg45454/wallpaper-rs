@fragment
fn fs_blur_motion(in: VertexOut) -> @location(0) vec4<f32> {
    let a = radians(param(0u).x);
    let most = param(1u).x;
    let moved = fx.velocity.xy * param(4u).x;
    let along = moved * min(1.0, most / max(length(moved), 1e-5));
    let aimed = select(vec2<f32>(cos(a), sin(a)) * most, along, param(3u).x > 0.5);
    let span = px(mask_at(in.uv)) * aimed;
    let n = clamp(i32(param(2u).x), 2, 32);
    let jitter = hash(in.position.xy) - 0.5;
    var sum = vec4<f32>(0.0);
    for (var i = 0; i < n; i++) {
        sum += tap(in.uv + span * ((f32(i) + 0.5 + jitter) / f32(n) - 0.5));
    }
    return sum / f32(n);
}
