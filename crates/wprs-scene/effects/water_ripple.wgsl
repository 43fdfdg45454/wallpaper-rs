fn water_ripple_drop(uv: vec2<f32>, at: vec2<f32>, amount: f32) -> f32 {
    let d = (uv - at) / px(param(2u).x);
    return amount * exp(-dot(d, d));
}

fn water_ripple_height(uv: vec2<f32>) -> f32 {
    return sample(aux0, uv).r;
}

@fragment
fn fs_water_ripple_step(in: VertexOut) -> @location(0) vec4<f32> {
    let s = tap(in.uv);
    let speed = param(11u).x;
    let e = vec3<f32>(fx.texel.xy * max(speed, 1.0), 0.0);
    let around = tap(in.uv - e.xz).r + tap(in.uv + e.xz).r + tap(in.uv - e.zy).r + tap(in.uv + e.zy).r;
    let first = f32(fx.info.y < 0.5);
    let pressed = f32(frame.buttons > 0.0 && frame.inside > 0.0);
    let swipe = min(length(frame.cursor_delta), 1.0) * frame.inside * min(frame.dt * 60.0, 4.0) * 0.1 * first;
    let poke = (param(3u).x * swipe + param(4u).x * max(pressed - s.a, 0.0)) * 0.5;
    let drop = from_layer(hash2(vec2<f32>(frame.time, frame.seed) * 7.31));
    let c2 = min(speed, 1.0) * min(speed, 1.0);
    var h = (2.0 * s.r - s.g + c2 * (around * 0.5 - 2.0 * s.r)) * param(1u).x * param(1u).x;
    h += water_ripple_drop(in.uv, cursor_uv(), poke);
    h += water_ripple_drop(in.uv, drop, param(5u).x * max(frame.beat - s.b, 0.0) * 0.25);
    let wall = asset_at(aux0, in.uv);
    return vec4<f32>(h * (1.0 - wall.r * wall.a), s.r, frame.beat, pressed);
}

@fragment
fn fs_water_ripple(in: VertexOut) -> @location(0) vec4<f32> {
    let e = vec3<f32>(1.0 / vec2<f32>(textureDimensions(aux0)), 0.0);
    let dx = water_ripple_height(in.uv + e.xz) - water_ripple_height(in.uv - e.xz);
    let dy = water_ripple_height(in.uv + e.zy) - water_ripple_height(in.uv - e.zy);
    let far = mix(1.0, 0.3 + 0.7 * clamp(layer_uv(in.uv).y, 0.0, 1.0), param(7u).x);
    let g = vec2<f32>(dx, dy) * 0.5 * mask_at(in.uv) * far;
    let q = layer_uv(in.uv - g * px(param(0u).x));
    let inside = all(abs(layer_uv(in.uv) - 0.5) <= vec2<f32>(0.5));
    let c = tap(from_layer(select(q, clamp(q, vec2<f32>(0.0), vec2<f32>(1.0)), inside)));
    let light = clamp(dot(g, vec2<f32>(-0.6, -0.8)) * 8.0, 0.0, 1.0);
    let wet = min(length(g) * 16.0, 1.0);
    let seen = asset_at(aux1, in.uv - g * px(param(0u).x) * 4.0);
    let tint = mix(vec3<f32>(1.0), from_linear(param(9u).rgb), param(9u).a * wet);
    let rgb = c.rgb * tint + seen.rgb * seen.a * wet * c.a;
    return vec4<f32>(min(rgb + light * light * param(6u).x * c.a, vec3<f32>(c.a)), c.a);
}
