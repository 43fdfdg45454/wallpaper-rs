@fragment
fn fs_vhs(in: VertexOut) -> @location(0) vec4<f32> {
    let l = layer_uv(in.uv);
    let m = mask_at(in.uv) * param(0u).x;
    let t = frame.time * param(1u).x;
    let line = fract(t * 0.13);
    let near = 1.0 - smoothstep(0.0, 0.05, abs(l.y - line));
    let row = floor(l.y * 240.0);
    let tear = (hash(vec2<f32>(row, floor(t * 24.0))) - 0.5) * near * 24.0 + sin(l.y * 9.0 + t * 2.0) * 0.6;
    let shift = px(tear * m).x;
    let split = px(param(3u).x * m).x;
    let at = in.uv + vec2<f32>(shift, 0.0);
    let r = tap(at + vec2<f32>(split, 0.0));
    let g = tap(at);
    let b = tap(at - vec2<f32>(split, 0.0));
    let snow = (hash(floor(in.uv / px(1.0)) + floor(t * 30.0) * 7.3 + frame.seed) - 0.5) * param(2u).x * m;
    let a = max(g.a, max(r.a, b.a));
    let rgb = clamp(vec3<f32>(r.r, g.g, b.b) + (snow + near * 0.15 * m) * a, vec3<f32>(0.0), vec3<f32>(a));
    return vec4<f32>(rgb, a);
}
