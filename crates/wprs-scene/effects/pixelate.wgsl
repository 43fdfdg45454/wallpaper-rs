fn uv_pixelate(uv: vec2<f32>) -> vec2<f32> {
    let unit = px(1.0);
    let q = uv / unit;
    let s = max(param(0u).x, 1.0);
    let square = (floor(q / s) + 0.5) * s;
    let r = vec2<f32>(1.0, 1.7320508) * s;
    let a = q - r * (floor(q / r) + 0.5);
    let b = q - r * (floor(q / r - 0.5) + 1.0);
    let hex = q - select(b, a, dot(a, a) < dot(b, b));
    return select(square, hex, param(1u).x > 0.5) * unit;
}
