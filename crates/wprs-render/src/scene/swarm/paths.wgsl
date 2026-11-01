fn turn(count: f32, mirror: bool, order: u32) -> vec2<u32> {
    let n = max(u32(max(count, 1.0)), 1u);
    if !mirror || n < 2u {
        return vec2<u32>(order % n, order / n);
    }
    let period = 2u * (n - 1u);
    let j = order % period;
    return vec2<u32>(select(period - j, j, j < n), order / period);
}

fn taper(f: f32, at: f32, size: f32) -> f32 {
    let d = select((f - at) / max(1.0 - at, 1e-3), (at - f) / max(at, 1e-3), f < at);
    return lerp(1.0, size, clamp(d, 0.0, 1.0));
}

fn sweep(e: u32, id: u32, order: u32, kind: u32) -> array<vec2<f32>, 3> {
    let a = sim.emitters[e * 4u];
    let b = sim.emitters[e * 4u + 1u];
    if kind == 6u {
        let i = f32(turn(b.x, b.z > 0.5, order).x);
        let steps = select(b.x - 1.0, b.x, a.w >= 360.0);
        let angle = radians(i / max(steps, 1.0) * a.w + b.y + b.w);
        return array(a.xy + a.z * vec2<f32>(cos(angle), sin(angle)), a.xy, vec2<f32>(1.0, 0.0));
    }
    if kind == 7u {
        let r = turn(b.x, b.z > 0.5, order);
        let f = select(0.0, f32(r.x) / (b.x - 1.0), b.x > 1.5) * b.y;
        let p = sim.paths[e * 2u];
        let q = sim.paths[e * 2u + 1u];
        let d = a.zw - a.xy;
        let flip = select(1.0, -1.0, rand(r.y, KEY + 48u) < 0.5);
        let bow = p.x * sin(3.14159265 * f) * select(flip, p.y, p.y != 0.0);
        let at = a.xy + d * f + vec2<f32>(-d.y, d.x) * bow;
        let at_t = p.z + q.x;
        let grow = select(1.0, taper(f, at_t - floor(at_t), p.w), b.w > 0.5);
        return array(at, (a.xy + a.zw) / 2.0, vec2<f32>(grow, 0.0));
    }
    let n = u32(a.y);
    let k = min(u32(rand(id, 0u) * a.y), max(n, 1u) - 1u);
    let i = u32(a.x) + 2u * k;
    return array(points[i], vec2<f32>(0.0), vec2<f32>(1.0, points[i + 1u].x));
}
