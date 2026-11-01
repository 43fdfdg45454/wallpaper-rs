fn hash(p: vec2<f32>) -> f32 {
    var q = fract(p.xyx * 0.1031);
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn hash2(p: vec2<f32>) -> vec2<f32> {
    var q = fract(p.xyx * vec3<f32>(0.1031, 0.1030, 0.0973));
    q += dot(q, q.yzx + 33.33);
    return fract((q.xx + q.yz) * q.zy);
}

fn noise1(x: f32) -> f32 {
    let i = floor(x);
    let f = fract(x);
    return mix(hash(vec2<f32>(i, 0.0)), hash(vec2<f32>(i + 1.0, 0.0)), f * f * (3.0 - 2.0 * f));
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(hash(i), hash(i + vec2<f32>(1.0, 0.0)), u.x);
    return mix(a, mix(hash(i + vec2<f32>(0.0, 1.0)), hash(i + vec2<f32>(1.0)), u.x), u.y);
}

fn gradient_corner(i: vec2<f32>, f: vec2<f32>, o: vec2<f32>) -> f32 {
    return dot(hash2(i + o) * 2.0 - 1.0, f - o);
}

fn gradient_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let a = mix(gradient_corner(i, f, vec2<f32>(0.0)), gradient_corner(i, f, vec2<f32>(1.0, 0.0)), u.x);
    let b = mix(gradient_corner(i, f, vec2<f32>(0.0, 1.0)), gradient_corner(i, f, vec2<f32>(1.0)), u.x);
    return mix(a, b, u.y) * 1.4142;
}

fn fbm(p: vec2<f32>, octaves: i32) -> f32 {
    var sum = 0.0;
    var amp = 0.5;
    var q = p;
    for (var i = 0; i < min(octaves, 8); i++) {
        sum += amp * gradient_noise(q);
        q = mat2x2<f32>(1.6, 1.2, -1.2, 1.6) * q + 17.3;
        amp *= 0.5;
    }
    return sum;
}

