@fragment
fn fs_god_rays_bright(in: VertexOut) -> @location(0) vec4<f32> {
    let c = tap(in.uv);
    let t = param(1u).x;
    return c * smoothstep(t - 0.1, t + 0.1, dot(c.rgb, LUMA)) * mask_at(in.uv);
}

@fragment
fn fs_god_rays_march(in: VertexOut) -> @location(0) vec4<f32> {
    let source = mix(from_layer(param(0u).xy), cursor_uv(), param(7u).x * frame.presence);
    let n = clamp(i32(param(6u).x), 4, 16);
    let fine = f32(n * n);
    let steps = select(1.0, f32(n), fx.info.z > 1.5);
    let fade = pow(param(3u).x, 64.0 / fine * steps);
    let stride = (in.uv - source) * param(2u).x * steps / fine;
    var sum = vec4<f32>(0.0);
    var weight = 1.0;
    var total = 0.0;
    for (var i = 0; i < n; i++) {
        sum += tap(in.uv - stride * f32(i)) * weight;
        total += weight;
        weight *= fade;
    }
    return sum / total;
}

@fragment
fn fs_god_rays_mix(in: VertexOut) -> @location(0) vec4<f32> {
    let c = tap(in.uv);
    let rays = sample(aux0, in.uv).rgb * from_linear(param(5u).rgb) * param(5u).a * param(4u).x;
    return vec4<f32>(c.rgb + rays, min(max(c.a, max(rays.r, max(rays.g, rays.b))), 1.0));
}
