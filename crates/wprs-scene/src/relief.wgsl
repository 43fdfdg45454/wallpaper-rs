fn relief(t: texture_2d<f32>, s: sampler, uv: vec2<f32>, shift: vec2<f32>, focus: f32, steps: f32) -> vec2<f32> {
    var prev = uv - shift * (1.0 - focus);
    var last = textureSampleLevel(t, s, prev, 0.0).r - 1.0;
    if last >= 0.0 {
        return prev;
    }
    for (var i = 1.0; i <= steps; i += 1.0) {
        let h = 1.0 - i / steps;
        let q = uv - shift * (h - focus);
        let d = textureSampleLevel(t, s, q, 0.0).r - h;
        if d >= 0.0 {
            return mix(q, prev, d / (d - last));
        }
        prev = q;
        last = d;
    }
    return prev;
}
