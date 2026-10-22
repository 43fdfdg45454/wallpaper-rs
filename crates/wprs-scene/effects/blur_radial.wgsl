@fragment
fn fs_blur_radial(in: VertexOut) -> @location(0) vec4<f32> {
    let center = mix(from_layer(param(0u).xy), cursor_uv(), param(3u).x * frame.presence);
    let n = clamp(i32(param(2u).x), 2, 16);
    let span = param(1u).x * mask_at(in.uv) / select(f32(n * n), f32(n), fx.info.z > 0.5);
    var sum = vec4<f32>(0.0);
    for (var i = 0; i < n; i++) {
        sum += tap(center + (in.uv - center) * (1.0 - span * f32(i)));
    }
    return sum / f32(n);
}
