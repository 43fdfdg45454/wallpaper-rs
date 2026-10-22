@fragment
fn fs_perspective(in: VertexOut) -> @location(0) vec4<f32> {
    let l = vec3<f32>(layer_uv(in.uv), 1.0);
    let h = vec3<f32>(dot(param(4u).xyz, l), dot(param(5u).xyz, l), dot(param(6u).xyz, l));
    let s = h.xy / h.z;
    let edge = min(min(s.x, 1.0 - s.x), min(s.y, 1.0 - s.y));
    let inside = clamp(edge / max(fwidth(edge), 1e-6) + 0.5, 0.0, 1.0);
    let m = mask_at(in.uv);
    return tap(mix(in.uv, from_layer(clamp(s, vec2<f32>(0.0), vec2<f32>(1.0))), m)) * mix(1.0, inside, m);
}
