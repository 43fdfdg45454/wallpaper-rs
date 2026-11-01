@fragment
fn fs_blur_h(in: VertexOut) -> @location(0) vec4<f32> {
    return gauss(input_tex, in.uv, vec2<f32>(1.0, 0.0), param(0u).x);
}

@fragment
fn fs_blur_v(in: VertexOut) -> @location(0) vec4<f32> {
    let s = gauss(input_tex, in.uv, vec2<f32>(0.0, 1.0), param(0u).x);
    let mode = u32(param(1u).x + 0.5);
    let k = param(2u).x;
    if mode == 0u && k >= 1.0 {
        return s;
    }
    let b = sample(aux0, in.uv);
    let a = mix(b.a, select(max(b.a, s.a), s.a, mode == 0u), k);
    let rgb = clamp(blend_mode(mode, straight(b).rgb, straight(s).rgb, k), vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(rgb * a, a);
}
