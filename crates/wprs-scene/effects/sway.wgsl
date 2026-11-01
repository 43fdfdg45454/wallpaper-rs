fn uv_sway(uv: vec2<f32>) -> vec2<f32> {
    let l = layer_uv(uv);
    let wave = TAU * (param(1u).x * frame.time + time_phase(aux0, uv));
    let m = mask_at(uv);
    let twice = param(7u).x > 0.5;
    if param(4u).x > 0.5 {
        let unit = px(1.0);
        let p = from_layer(param(6u).xy);
        let swing = select(sin(wave), 0.65 * sin(wave) + 0.35 * sin(wave * 1.7 + 1.3), twice);
        let a = radians(param(5u).x) * swing * m;
        return p + mat2x2<f32>(cos(a), sin(a), -sin(a), cos(a)) * ((uv - p) / unit) * unit;
    }
    let lift = pow(clamp(1.0 - l.y, 0.0, 1.0), param(2u).x);
    let place = TAU * param(3u).x * (l.x + 0.5 * l.y);
    let one = sin(wave + place);
    let s = select(one, 0.65 * one + 0.35 * sin(wave * 1.7 + place * 0.6 + 1.3), twice);
    return uv - vec2<f32>(px(param(0u).x).x * s * lift * m, 0.0);
}
