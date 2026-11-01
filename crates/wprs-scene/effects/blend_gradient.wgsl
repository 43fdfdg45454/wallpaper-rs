@fragment
fn fs_blend_gradient(in: VertexOut) -> @location(0) vec4<f32> {
    let c = tap(in.uv);
    let m = asset_at(aux1, in.uv);
    let level = select(clamp(layer_uv(in.uv).x, 0.0, 1.0), m.r, m.a > 0.0);
    let s = param(3u).x;
    let w = smoothstep(level - s * 0.5 - 1e-4, level + s * 0.5, param(2u).x * (1.0 + s) - s * 0.5);
    return mix(c, asset_at(aux0, in.uv), w * mask_at(in.uv));
}
