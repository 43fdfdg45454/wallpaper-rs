fn fluid_reach(uv: vec2<f32>, at: vec2<f32>, k: vec2<f32>) -> f32 {
    let d = (uv - at) * k;
    return exp(-dot(d, d));
}

fn fluid_line(uv: vec2<f32>, k: vec2<f32>) -> f32 {
    let a = from_layer(param(9u).xy);
    let b = from_layer(param(10u).xy);
    let ab = b - a;
    let t = clamp(dot(uv - a, ab) / max(dot(ab, ab), 1e-8), 0.0, 1.0);
    return fluid_reach(uv, a + ab * t, k);
}

@fragment
fn fs_fluid_advect(in: VertexOut) -> @location(0) vec4<f32> {
    let now = tap(in.uv);
    let was = tap(in.uv - now.rg * fx.texel.xy);
    let turn = radians(param(7u).x);
    let up = vec2<f32>(cos(turn), sin(turn));
    let size = layer_extent();
    let k = 1.0 / (px(1.0) * max(max(size.x, size.y), 1.0) * 0.03);
    let spot = fluid_reach(in.uv, from_layer(param(6u).xy), k) * param(8u).x;
    let line = fluid_line(in.uv, k) * param(11u).x;
    let moved = frame.cursor_delta * min(frame.dt, 0.07) * frame.resolution / max(fx.area.zw, vec2<f32>(1.0)) * fx.texel.zw;
    let first = f32(fx.info.y < 0.5) * frame.inside * param(5u).x;
    let touch = fluid_reach(in.uv, cursor_uv(), k) * first;
    var vel = was.rg * param(3u).x + up * (spot + line) * 0.02 + moved * touch * 0.5;
    vel = clamp(vel, vec2<f32>(-8.0), vec2<f32>(8.0));
    let dye = min(was.b * param(4u).x + (spot + line) * 0.02 + touch * min(length(moved) * 0.2, 1.0), 4.0);
    return vec4<f32>(vel, dye, now.a);
}

@fragment
fn fs_fluid_divergence(in: VertexOut) -> @location(0) vec4<f32> {
    let e = vec3<f32>(fx.texel.xy, 0.0);
    let d = tap(in.uv + e.xz).r - tap(in.uv - e.xz).r + tap(in.uv + e.zy).g - tap(in.uv - e.zy).g;
    return vec4<f32>(d * 0.5);
}

@fragment
fn fs_fluid_pressure(in: VertexOut) -> @location(0) vec4<f32> {
    if fx.info.z > select(11.0, 5.0, frame.quality < 1.5) {
        return tap(in.uv);
    }
    let e = vec3<f32>(fx.texel.xy, 0.0);
    let f = e * 2.0;
    let far = tap(in.uv - f.xz).a + tap(in.uv + f.xz).a + tap(in.uv - f.zy).a + tap(in.uv + f.zy).a;
    let corners = tap(in.uv + e.xy).a + tap(in.uv - e.xy).a + tap(in.uv + vec2<f32>(e.x, -e.y)).a
        + tap(in.uv - vec2<f32>(e.x, -e.y)).a;
    let near = sample(aux0, in.uv - e.xz).r + sample(aux0, in.uv + e.xz).r + sample(aux0, in.uv - e.zy).r
        + sample(aux0, in.uv + e.zy).r;
    return vec4<f32>((4.0 * tap(in.uv).a + far + 2.0 * corners - near) / 16.0 - sample(aux0, in.uv).r * 0.25);
}

@fragment
fn fs_fluid_step(in: VertexOut) -> @location(0) vec4<f32> {
    let now = tap(in.uv);
    let e = vec3<f32>(1.0 / vec2<f32>(textureDimensions(aux0)), 0.0);
    let grad = vec2<f32>(sample(aux0, in.uv + e.xz).a - sample(aux0, in.uv - e.xz).a,
        sample(aux0, in.uv + e.zy).a - sample(aux0, in.uv - e.zy).a) * 0.5;
    return vec4<f32>(now.rg - grad, now.b, sample(aux0, in.uv).a);
}

@fragment
fn fs_fluid(in: VertexOut) -> @location(0) vec4<f32> {
    let c = tap(in.uv);
    let y = mix(in.uv.y, in.uv.y * in.uv.y, param(13u).x);
    let s = sample(aux0, vec2<f32>(in.uv.x, y));
    let m = mask_at(in.uv);
    let drift = s.rg / vec2<f32>(textureDimensions(aux0)) * 8.0 * m;
    let mode = u32(param(0u).x + 0.5);
    if mode == 3u {
        return tap(in.uv - drift);
    }
    let dye = clamp(s.b, 0.0, 1.0) * m;
    var tone = from_linear(param(2u).rgb) * param(2u).a;
    if mode == 0u && textureDimensions(aux1).x > 1u {
        tone = textureSampleLevel(aux1, samp, vec2<f32>(dye, 0.5), 0.0).rgb;
    }
    if mode == 2u {
        tone = straight(tap(in.uv - drift)).rgb;
    }
    let opaque = param(14u).x > 0.5;
    let a = select(max(c.a, dye), 1.0, opaque);
    let base = select(tone, straight(c).rgb, c.a > 1e-4 || opaque);
    let rgb = clamp(blend_mode(u32(param(12u).x + 0.5), base, tone, dye), vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(rgb * a, a);
}
