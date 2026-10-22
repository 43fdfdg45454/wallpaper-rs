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
    let e = vec3<f32>(fx.texel.xy, 0.0);
    let around = tap(in.uv - e.xz).r + tap(in.uv + e.xz).r + tap(in.uv - e.zy).r + tap(in.uv + e.zy).r;
    let first = f32(fx.info.y < 0.5);
    let pressed = f32(frame.buttons > 0.0 && frame.inside > 0.0);
    let swipe = min(length(frame.cursor_delta), 1.0) * frame.inside * min(frame.dt * 60.0, 4.0) * 0.1 * first;
    let poke = (param(3u).x * swipe + param(4u).x * max(pressed - s.a, 0.0)) * 0.5;
    let drop = from_layer(hash2(vec2<f32>(frame.time, frame.seed) * 7.31));
    var h = (around * 0.5 - s.g) * param(1u).x * param(1u).x;
    h += water_ripple_drop(in.uv, cursor_uv(), poke);
    h += water_ripple_drop(in.uv, drop, param(5u).x * max(frame.beat - s.b, 0.0) * 0.25);
    return vec4<f32>(h, s.r, frame.beat, pressed);
}

@fragment
fn fs_water_ripple(in: VertexOut) -> @location(0) vec4<f32> {
    let e = vec3<f32>(1.0 / vec2<f32>(textureDimensions(aux0)), 0.0);
    let dx = water_ripple_height(in.uv + e.xz) - water_ripple_height(in.uv - e.xz);
    let dy = water_ripple_height(in.uv + e.zy) - water_ripple_height(in.uv - e.zy);
    let g = vec2<f32>(dx, dy) * 0.5 * mask_at(in.uv);
    let q = layer_uv(in.uv - g * px(param(0u).x));
    let inside = all(abs(layer_uv(in.uv) - 0.5) <= vec2<f32>(0.5));
    let c = tap(from_layer(select(q, clamp(q, vec2<f32>(0.0), vec2<f32>(1.0)), inside)));
    let light = clamp(dot(g, vec2<f32>(-0.6, -0.8)) * 8.0, 0.0, 1.0);
    return vec4<f32>(min(c.rgb + light * light * param(6u).x * c.a, vec3<f32>(c.a)), c.a);
}
