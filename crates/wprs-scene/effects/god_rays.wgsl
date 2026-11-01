@fragment
fn fs_god_rays_bright(in: VertexOut) -> @location(0) vec4<f32> {
    let c = tap(in.uv);
    let t = param(1u).x;
    return c * smoothstep(t - 0.1, t + 0.1, dot(c.rgb, LUMA)) * mask_at(in.uv);
}

fn god_rays_along(uv: vec2<f32>, stride: vec2<f32>, n: i32, fade: f32) -> vec4<f32> {
    var sum = vec4<f32>(0.0);
    var weight = 1.0;
    var total = 0.0;
    for (var i = 0; i < n; i++) {
        sum += tap(uv - stride * f32(i)) * weight;
        total += weight;
        weight *= fade;
    }
    return sum / total;
}

@fragment
fn fs_god_rays_march(in: VertexOut) -> @location(0) vec4<f32> {
    let source = mix(from_layer(param(0u).xy), cursor_uv(), param(7u).x * frame.presence);
    let n = clamp(i32(param(6u).x), 4, 16);
    let fine = f32(n * n);
    let steps = select(1.0, f32(n), fx.info.z > 1.5);
    let fade = pow(param(3u).x, 64.0 / fine * steps);
    let mode = u32(param(8u).x + 0.5);
    let a = radians(param(9u).x);
    let way = vec2<f32>(cos(a), sin(a)) * 0.5 * max(fx.texel.z, fx.texel.w) * fx.texel.xy;
    if mode == 1u {
        return god_rays_along(in.uv, way * param(2u).x * steps / fine, n, fade);
    }
    if mode == 2u {
        if fx.info.z > 1.5 {
            return tap(in.uv);
        }
        let edges = clamp(i32(param(11u).x), 2, 16);
        let reach = 0.15 * max(fx.texel.z, fx.texel.w) * fx.texel.xy * param(2u).x / f32(2 * n);
        let star = pow(param(3u).x, 32.0 / f32(n));
        var sum = vec4<f32>(0.0);
        for (var k = 0; k < edges; k++) {
            let turn = a + TAU * f32(k) / f32(edges);
            sum += god_rays_along(in.uv, vec2<f32>(cos(turn), sin(turn)) * reach, 2 * n, star);
        }
        return sum * 2.0 / f32(edges);
    }
    return god_rays_along(in.uv, (in.uv - source) * param(2u).x * steps / fine, n, fade);
}

@fragment
fn fs_god_rays_mix(in: VertexOut) -> @location(0) vec4<f32> {
    let c = tap(in.uv);
    let l = layer_uv(in.uv);
    let shimmer = mix(1.0, value_noise(l * 6.0 + vec2<f32>(frame.time * 1.7, frame.time * 1.3)) * 1.6, param(10u).x);
    let rays = sample(aux0, in.uv).rgb * from_linear(param(5u).rgb) * param(5u).a * param(4u).x * shimmer;
    return vec4<f32>(c.rgb + rays, min(max(c.a, max(rays.r, max(rays.g, rays.b))), 1.0));
}
