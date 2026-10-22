@fragment
fn fs_chromatic_aberration(in: VertexOut) -> @location(0) vec4<f32> {
    let d = (layer_uv(in.uv) - param(1u).xy) * 2.0 * px(param(0u).x * mask_at(in.uv));
    let r = tap(in.uv + d);
    let g = tap(in.uv);
    let b = tap(in.uv - d);
    return vec4<f32>(r.r, g.g, b.b, max(g.a, max(r.a, b.a)));
}
