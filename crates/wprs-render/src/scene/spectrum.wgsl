fn band(i: f32) -> f32 {
    let n = clamp(i * 64.0 / item.mode.z, 0.0, 63.0);
    let v = frame.audio[u32(n) / 4u][u32(n) % 4u];
    return clamp(v, 0.0, 1.0);
}

@fragment
fn fs_spectrum(in: Out) -> @location(0) vec4<f32> {
    let n = item.mode.z;
    var uv = in.uv;
    if item.mode.y > 0.5 && item.mode.y < 1.5 {
        let p = in.uv - 0.5;
        uv = vec2<f32>(atan2(p.x, -p.y) / (2.0 * PI) + 0.5, 1.0 - (length(p) * 2.0 - 0.4) / 0.6);
    }
    let i = floor(uv.x * n);
    let v = band(i);
    var d: f32;
    if item.mode.y > 1.5 {
        let x = uv.x * n - 0.5;
        let level = mix(band(floor(x)), band(floor(x) + 1.0), fract(x));
        d = (abs(uv.y - (1.0 - level * 0.9 - 0.05)) - 0.02) * item.size.y;
    } else {
        let gap = item.mode.w * 0.5;
        let fx = abs(fract(uv.x * n) - 0.5) - (0.5 - gap);
        d = max(fx * item.size.x / n, ((1.0 - v) - uv.y) * item.size.y);
        d = max(d, max(-uv.y, uv.y - 1.0) * item.size.y);
    }
    return finish(item.color * coverage(d), in);
}
