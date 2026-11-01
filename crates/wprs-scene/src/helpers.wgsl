const OUTSIDE_UV: vec2<f32> = vec2<f32>(-65536.0, -65536.0);

fn outside_layer(uv: vec2<f32>) -> vec2<f32> {
    let l = layer_uv(uv);
    return select(OUTSIDE_UV, uv, all(l >= vec2<f32>(0.0)) && all(l <= vec2<f32>(1.0)));
}

fn layer_extent() -> vec2<f32> {
    let o = from_layer(vec2<f32>(0.0));
    let unit = px(1.0);
    return vec2<f32>(length((from_layer(vec2<f32>(1.0, 0.0)) - o) / unit), length((from_layer(vec2<f32>(0.0, 1.0)) - o) / unit));
}

fn time_phase(t: texture_2d<f32>, uv: vec2<f32>) -> f32 {
    let p = asset_at(t, uv);
    return p.r * p.a;
}

fn gauss(t: texture_2d<f32>, uv: vec2<f32>, dir: vec2<f32>, radius: f32) -> vec4<f32> {
    let r = max(radius * fx.info.w, 0.0);
    if r < 0.5 {
        return sample(t, uv);
    }
    let sigma = max(r / 2.5, 0.5);
    var sum = vec4<f32>(0.0);
    var total = 0.0;
    for (var i = -8; i <= 8; i++) {
        let x = f32(i) * r / 8.0;
        let w = exp(-x * x / (2.0 * sigma * sigma));
        sum += sample(t, uv + dir * x * fx.texel.xy) * w;
        total += w;
    }
    return sum / total;
}

