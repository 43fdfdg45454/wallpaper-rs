fn tilt_shift_blur(uv: vec2<f32>, depth: texture_2d<f32>) -> f32 {
    let a = radians(param(3u).x);
    let band = dot(layer_uv(uv) - 0.5, vec2<f32>(-sin(a), cos(a))) + 0.5;
    let at = select(band, asset_at(depth, uv).r, textureDimensions(depth).x > 1u);
    let d = abs(at - param(0u).x);
    return smoothstep(param(1u).x, param(1u).x + param(2u).x + 1e-4, d) * mask_at(uv);
}

@fragment
fn fs_tilt_shift_h(in: VertexOut) -> @location(0) vec4<f32> {
    return gauss(input_tex, in.uv, vec2<f32>(1.0, 0.0), param(4u).x * tilt_shift_blur(in.uv, aux0));
}

@fragment
fn fs_tilt_shift_v(in: VertexOut) -> @location(0) vec4<f32> {
    return gauss(input_tex, in.uv, vec2<f32>(0.0, 1.0), param(4u).x * tilt_shift_blur(in.uv, aux0));
}

@fragment
fn fs_tilt_shift_mix(in: VertexOut) -> @location(0) vec4<f32> {
    return mix(tap(in.uv), sample(aux0, in.uv), min(tilt_shift_blur(in.uv, aux1) * 2.0, 1.0));
}
