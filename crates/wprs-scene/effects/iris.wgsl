fn iris_start(k: f32) -> f32 {
    return k + (hash(vec2<f32>(k, frame.seed + 7.0)) - 0.5) * 0.7;
}

fn iris_target(k: f32) -> vec2<f32> {
    let g = hash2(vec2<f32>(k, frame.seed)) * 2.0 - 1.0;
    return g * 0.8 / max(length(g), 1.0);
}

fn iris_gaze() -> vec2<f32> {
    let t = frame.time * param(4u).x;
    var k = floor(t);
    k += select(0.0, -1.0, t < iris_start(k)) + select(0.0, 1.0, t >= iris_start(k + 1.0));
    let jump = (t - iris_start(k)) / ((0.03 + 0.03 * hash(vec2<f32>(k, frame.seed + 3.0))) * param(4u).x);
    return mix(iris_target(k - 1.0), iris_target(k), smoothstep(0.0, 1.0, jump));
}

fn uv_iris(uv: vec2<f32>) -> vec2<f32> {
    let unit = px(1.0);
    let range = param(0u).x;
    let look = (cursor_uv() - from_layer(param(2u).xy)) / unit * param(1u).x;
    let follow = look * min(1.0, range / max(length(look), 1e-3));
    let offset = select(iris_gaze() * range * param(3u).x, follow, frame.inside > 0.5);
    return uv - offset * unit * mask_at(uv);
}
