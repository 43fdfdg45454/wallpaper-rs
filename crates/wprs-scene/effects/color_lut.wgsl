fn color_lut_srgb(c: vec3<f32>, encode: bool) -> vec3<f32> {
    let x = max(c, vec3<f32>(0.0));
    let e = select(1.055 * pow(x, vec3<f32>(1.0 / 2.4)) - 0.055, x * 12.92, x <= vec3<f32>(0.0031308));
    let d = select(pow((x + 0.055) / 1.055, vec3<f32>(2.4)), x / 12.92, x <= vec3<f32>(0.04045));
    return select(select(d, e, encode), c, frame.srgb > 0.5);
}

@fragment
fn fs_color_lut(in: VertexOut) -> @location(0) vec4<f32> {
    let c = straight(tap(in.uv));
    let graded = color_lut_srgb(lut(color_lut_srgb(c.rgb, true)), false);
    let k = param(1u).x * mask_at(in.uv) * f32(textureDimensions(lut_tex).x > 1u);
    return vec4<f32>(mix(c.rgb, graded, k) * c.a, c.a);
}
