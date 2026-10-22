@fragment
fn fs_blur_h(in: VertexOut) -> @location(0) vec4<f32> {
    return gauss(input_tex, in.uv, vec2<f32>(1.0, 0.0), param(0u).x);
}

@fragment
fn fs_blur_v(in: VertexOut) -> @location(0) vec4<f32> {
    return gauss(input_tex, in.uv, vec2<f32>(0.0, 1.0), param(0u).x);
}
