@fragment
fn fs_blend_texture(in: VertexOut) -> @location(0) vec4<f32> {
    let c = straight(tap(in.uv));
    let at = (layer_uv(in.uv) - 0.5 - param(3u).xy) / max(param(4u).xy, vec2<f32>(0.01)) + 0.5;
    let t = straight(textureSampleLevel(aux0, wrap, at, 0.0));
    let mixed = clamp(blend_pick(u32(param(1u).x + 0.5), c.rgb, t.rgb), vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(mix(c.rgb, mixed, param(2u).x * t.a * mask_at(in.uv)) * c.a, c.a);
}
