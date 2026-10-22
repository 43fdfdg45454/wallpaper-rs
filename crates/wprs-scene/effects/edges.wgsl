fn edges_luma(uv: vec2<f32>) -> f32 {
    let c = straight(tap(uv));
    return sqrt(max(dot(to_linear(c.rgb), LUMA), 0.0)) * c.a;
}

@fragment
fn fs_edges(in: VertexOut) -> @location(0) vec4<f32> {
    let o = px(param(2u).x);
    var l: array<f32, 9>;
    for (var i = 0; i < 9; i++) {
        l[i] = edges_luma(in.uv + vec2<f32>(f32(i % 3 - 1), f32(i / 3 - 1)) * o);
    }
    let gx = l[2] + 2.0 * l[5] + l[8] - l[0] - 2.0 * l[3] - l[6];
    let gy = l[6] + 2.0 * l[7] + l[8] - l[0] - 2.0 * l[1] - l[2];
    let t = param(1u).x;
    let e = smoothstep(t, t + 0.25, length(vec2<f32>(gx, gy))) * param(0u).x * mask_at(in.uv);
    let c = tap(in.uv);
    let base = select(c, vec4<f32>(from_linear(param(5u).rgb), 1.0) * c.a, param(4u).x > 0.5);
    return mix(base, vec4<f32>(from_linear(param(3u).rgb), 1.0), e * param(3u).a);
}
