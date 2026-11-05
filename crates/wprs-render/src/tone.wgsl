fn pq(e: vec3<f32>) -> vec3<f32> {
    let p = pow(max(e, vec3(0.0)), vec3(1.0 / 78.84375));
    return 10000.0 * pow(max(p - 0.8359375, vec3(0.0)) / (18.8515625 - 18.6875 * p), vec3(1.0 / 0.1593017578125));
}

fn hlg(e: vec3<f32>) -> vec3<f32> {
    let high = (exp((e - 0.55991073) / 0.17883277) + 0.28466892) / 12.0;
    let lin = select(high, e * e / 3.0, e <= vec3(0.5));
    let y = dot(lin, vec3(0.2627, 0.6780, 0.0593));
    return 1000.0 * lin * pow(max(y, 1e-6), 0.2);
}

fn mapped(e: vec3<f32>, tone: vec4<f32>, m0: vec3<f32>, m1: vec3<f32>, m2: vec3<f32>) -> vec3<f32> {
    let light = select(pq(e), hlg(e), tone.x > 1.5) / tone.y;
    let c = max(vec3(dot(m0, light), dot(m1, light), dot(m2, light)), vec3(0.0));
    let k = tone.w;
    let l = max(max(c.r, c.g), c.b);
    let x = max(l - k, 0.0) / (1.0 - k);
    let wx = (tone.z / tone.y - k) / (1.0 - k);
    let m = select(l, k + (1.0 - k) * x * (1.0 + x / (wx * wx)) / (1.0 + x), l > k);
    return clamp(c * (m / max(l, 1e-6)), vec3(0.0), vec3(1.0));
}
