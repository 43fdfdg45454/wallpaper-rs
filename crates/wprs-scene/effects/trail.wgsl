@fragment
fn fs_trail_step(in: VertexOut) -> @location(0) vec4<f32> {
    let now = tap(in.uv);
    let kept = sample(aux0, in.uv);
    let next = mix(now, kept, param(0u).x);
    let near = all(abs(next - now) < vec4<f32>(1.0 / 512.0));
    return select(next, now, near || kept.a <= 0.0);
}

@fragment
fn fs_trail(in: VertexOut) -> @location(0) vec4<f32> {
    return tap(in.uv);
}
