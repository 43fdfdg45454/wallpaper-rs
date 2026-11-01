@fragment
fn fs_fire(in: VertexOut) -> @location(0) vec4<f32> {
    let l = layer_uv(in.uv);
    let m = mask_at(in.uv);
    let t = frame.time * param(0u).x;
    let p = l * param(2u).x * vec2<f32>(2.0, 1.0) + vec2<f32>(0.0, t) + frame.seed;
    let flick = vec2<f32>(fbm(p * 2.0, 3) * 0.5, -abs(fbm(p * 1.5 + 9.3, 3)) - 0.15);
    let c = tap(in.uv - flick * px(param(1u).x) * m);
    if textureDimensions(aux0).x <= 1u {
        return c;
    }
    let s = straight(c);
    let heat = clamp(dot(s.rgb, LUMA) * (1.0 + 0.3 * fbm(p * 3.0, 2)), 0.0, 1.0);
    let ramp = textureSampleLevel(aux0, samp, vec2<f32>(heat, 0.5), 0.0);
    return vec4<f32>(mix(s.rgb, ramp.rgb, ramp.a * m) * c.a, c.a);
}
