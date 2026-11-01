fn fract(x: f32) -> f32 {
    x - x.floor()
}

fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn hash([x, y]: [f32; 2]) -> f32 {
    let q = [x, y, x].map(|v| fract(v * 0.1031));
    let d = q[0] * (q[1] + 33.33) + q[1] * (q[2] + 33.33) + q[2] * (q[0] + 33.33);
    let q = q.map(|v| v + d);
    fract((q[0] + q[1]) * q[2])
}

fn hash2([x, y]: [f32; 2]) -> [f32; 2] {
    let k = [0.1031, 0.1030, 0.0973];
    let q = [x * k[0], y * k[1], x * k[2]].map(fract);
    let d = q[0] * (q[1] + 33.33) + q[1] * (q[2] + 33.33) + q[2] * (q[0] + 33.33);
    let q = q.map(|v| v + d);
    [fract((q[0] + q[1]) * q[2]), fract((q[0] + q[2]) * q[1])]
}

pub fn noise1(x: f32) -> f32 {
    let (i, f) = (x.floor(), fract(x));
    mix(hash([i, 0.0]), hash([i + 1.0, 0.0]), f * f * (3.0 - 2.0 * f))
}

fn corner(i: [f32; 2], f: [f32; 2], o: [f32; 2]) -> f32 {
    let h = hash2([i[0] + o[0], i[1] + o[1]]);
    (h[0] * 2.0 - 1.0) * (f[0] - o[0]) + (h[1] * 2.0 - 1.0) * (f[1] - o[1])
}

pub fn gradient(p: [f32; 2]) -> f32 {
    let (i, f) = (p.map(f32::floor), p.map(fract));
    let u = f.map(|f| f * f * f * (f * (f * 6.0 - 15.0) + 10.0));
    let a = mix(corner(i, f, [0.0, 0.0]), corner(i, f, [1.0, 0.0]), u[0]);
    let b = mix(corner(i, f, [0.0, 1.0]), corner(i, f, [1.0, 1.0]), u[0]);
    mix(a, b, u[1]) * f32::from_bits(0x3fb5_0481)
}

pub fn fbm(p: [f32; 2], octaves: i32) -> f32 {
    let (mut sum, mut amp, mut q) = (0.0, 0.5, p);
    for _ in 0..octaves.min(8) {
        sum += amp * gradient(q);
        q = [1.6 * q[0] - 1.2 * q[1] + 17.3, 1.2 * q[0] + 1.6 * q[1] + 17.3];
        amp *= 0.5;
    }
    sum
}
