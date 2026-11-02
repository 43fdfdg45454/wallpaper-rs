@fragment
fn fs_air(in: Out) -> @location(0) vec4<f32> {
    let b = lights.bulbs[u32(item.lit_at.w + 0.5)];
    let p = item.lit_at.xy + in.position.xy * item.lit_at.z;
    let n = max(4u, u32(b.air.y * item.lit.x + 0.5));
    let reach_of = b.air.x;
    var sum = vec3<f32>(0.0);
    for (var i = 0u; i < n; i++) {
        let q = mix(p, b.at.xy, reach_of * f32(i) / f32(n));
        let r = reach(b, q);
        if r.w > 0.0 {
            sum += gobo(b, r.xyz) * r.w;
        }
    }
    let d = length(p - b.at.xy) / max(b.at.w, 1e-3);
    let edge = clamp(1.0 - d * d, 0.0, 1.0);
    return vec4<f32>(b.color.rgb * sum / f32(n) * b.air.x * edge, 0.0);
}

@fragment
fn fs_refract(in: Out) -> @location(0) vec4<f32> {
    let d = textureSampleLevel(mask_tex, samp, in.uv, 0.0);
    return textureSampleLevel(src_tex, samp, in.uv + d.xy * item.lit_env.zw, 0.0);
}
