@fragment
fn fs_bloom_bright(in: VertexOut) -> @location(0) vec4<f32> {
    let c = tap(in.uv);
    let l = max(c.r, max(c.g, c.b));
    let t = param(0u).x;
    let k = max(t * param(3u).x, 1e-4);
    let soft = clamp(l - t + k, 0.0, 2.0 * k);
    let gain = max(soft * soft / (4.0 * k), l - t) / max(l, 1e-4);
    return c * max(gain, 0.0);
}

@fragment
fn fs_bloom_h(in: VertexOut) -> @location(0) vec4<f32> {
    return gauss(input_tex, in.uv, vec2<f32>(1.0, 0.0), param(2u).x);
}

@fragment
fn fs_bloom_v(in: VertexOut) -> @location(0) vec4<f32> {
    return gauss(input_tex, in.uv, vec2<f32>(0.0, 1.0), param(2u).x);
}

@fragment
fn fs_bloom_mix(in: VertexOut) -> @location(0) vec4<f32> {
    return tap(in.uv) + sample(aux0, in.uv) * param(1u).x;
}
