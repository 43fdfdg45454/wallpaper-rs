fn chromatic_aberration_way(l: vec2<f32>) -> vec2<f32> {
    let d = l - param(1u).xy;
    let a = radians(param(3u).x);
    switch u32(param(2u).x + 0.5) {
        case 1u: { return vec2<f32>(cos(a), sin(a)); }
        case 2u: { return select(vec2<f32>(0.0), normalize(d), dot(d, d) > 1e-8); }
        case 3u: { return d * 8.0 * dot(d, d); }
        default: { return d * 2.0; }
    }
}

@fragment
fn fs_chromatic_aberration(in: VertexOut) -> @location(0) vec4<f32> {
    let d = chromatic_aberration_way(layer_uv(in.uv)) * px(param(0u).x * mask_at(in.uv));
    let r = tap(in.uv + d);
    let g = tap(in.uv);
    let b = tap(in.uv - d);
    return vec4<f32>(r.r, g.g, b.b, max(g.a, max(r.a, b.a)));
}
