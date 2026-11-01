@fragment
fn fs_local_contrast_h(in: VertexOut) -> @location(0) vec4<f32> {
    return gauss(input_tex, in.uv, vec2<f32>(1.0, 0.0), param(0u).x);
}

@fragment
fn fs_local_contrast_v(in: VertexOut) -> @location(0) vec4<f32> {
    return gauss(input_tex, in.uv, vec2<f32>(0.0, 1.0), param(0u).x);
}

@fragment
fn fs_local_contrast(in: VertexOut) -> @location(0) vec4<f32> {
    let c = tap(in.uv);
    let d = (c.rgb - sample(aux0, in.uv).rgb) * param(1u).x * mask_at(in.uv);
    let lit = select(d, vec3<f32>(dot(d, LUMA)), param(2u).x > 0.5);
    return vec4<f32>(clamp(c.rgb + lit, vec3<f32>(0.0), vec3<f32>(c.a)), c.a);
}
