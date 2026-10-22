@fragment
fn fs_blur_motion(in: VertexOut) -> @location(0) vec4<f32> {
    let a = radians(param(0u).x);
    let span = px(param(1u).x * mask_at(in.uv)) * vec2<f32>(cos(a), sin(a));
    let n = clamp(i32(param(2u).x), 2, 32);
    let jitter = hash(in.position.xy) - 0.5;
    var sum = vec4<f32>(0.0);
    for (var i = 0; i < n; i++) {
        sum += tap(in.uv + span * ((f32(i) + 0.5 + jitter) / f32(n) - 0.5));
    }
    return sum / f32(n);
}
