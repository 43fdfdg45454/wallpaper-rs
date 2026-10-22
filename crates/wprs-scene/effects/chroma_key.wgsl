fn color_chroma_key(color: vec4<f32>, uv: vec2<f32>) -> vec4<f32> {
    let c = straight(color);
    let lab = oklab(to_linear(c.rgb));
    let key = oklab(param(0u).rgb).yz;
    let d = distance(lab.yz, key);
    let t = param(1u).x;
    let m = mask_at(uv);
    let alpha = mix(1.0, smoothstep(t, t + param(2u).x + 1e-4, d), m);
    let dir = key / max(length(key), 1e-4);
    let near = 1.0 - smoothstep(t, t + param(2u).x + 0.2, d);
    let ab = lab.yz - dir * max(dot(lab.yz, dir), 0.0) * param(3u).x * near * m;
    let rgb = from_linear(max(from_oklab(vec3<f32>(lab.x, ab)), vec3<f32>(0.0)));
    return vec4<f32>(rgb, 1.0) * c.a * alpha;
}
