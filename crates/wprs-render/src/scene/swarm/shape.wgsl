fn middle(a: vec4<f32>) -> vec2<f32> {
    return (a.xy + a.zw) / 2.0;
}

fn hollow(a: vec4<f32>, b: vec4<f32>, u: vec2<f32>, pick: f32) -> vec2<f32> {
    let lo = min(a.xy, a.zw);
    let hi = max(a.xy, a.zw);
    let size = hi - lo;
    let inner = min(b.xy, size);
    let band = (size - inner) / 2.0;
    let row = size.x * band.y;
    let side = band.x * inner.y;
    let at = pick * 2.0 * (row + side);
    let y = vec2<f32>(lo.y + band.y, hi.y - band.y);
    if at < row {
        return vec2<f32>(lerp(lo.x, hi.x, u.x), lerp(lo.y, y.x, u.y));
    }
    if at < 2.0 * row {
        return vec2<f32>(lerp(lo.x, hi.x, u.x), lerp(y.y, hi.y, u.y));
    }
    if at < 2.0 * row + side {
        return vec2<f32>(lerp(lo.x, lo.x + band.x, u.x), lerp(y.x, y.y, u.y));
    }
    return vec2<f32>(lerp(hi.x - band.x, hi.x, u.x), lerp(y.x, y.y, u.y));
}

fn place(e: u32, id: u32, order: u32) -> array<vec2<f32>, 3> {
    let kind = sim.shapes[e / 4u][e % 4u];
    if kind >= 6u {
        return sweep(e, id, order, kind);
    }
    let at = spread(e, id);
    return array(at[0], at[1], vec2<f32>(1.0, 0.0));
}

fn spread(e: u32, id: u32) -> array<vec2<f32>, 2> {
    let a = sim.emitters[e * 4u];
    let b = sim.emitters[e * 4u + 1u];
    let u = vec2<f32>(rand(id, 0u), rand(id, 1u));
    let w = sim.clock.z;
    let h = sim.clock.w;
    let canvas = vec2<f32>(w, h) / 2.0;
    switch sim.shapes[e / 4u][e % 4u] {
        case 1u: {
            if b.x <= 0.0 && b.y <= 0.0 {
                return array(vec2<f32>(lerp(a.x, a.z, u.x), lerp(a.y, a.w, u.y)), middle(a));
            }
            return array(hollow(a, b, u, rand(id, KEY + 8u)), middle(a));
        }
        case 2u: {
            let r = sqrt(lerp(a.w * a.w, a.z * a.z, u.x));
            let t = TAU * u.y;
            var off = vec2<f32>(r * cos(t) * b.x, r * sin(t) * b.y);
            off = select(off, b.zw * abs(off), b.zw != vec2<f32>(0.0));
            return array(a.xy + off, a.xy);
        }
        case 3u: {
            return array(vec2<f32>(lerp(a.x, a.z, u.x), lerp(a.y, a.w, u.x)), middle(a));
        }
        case 4u: {
            let d = u.x * 2.0 * (w + h);
            var at = vec2<f32>(0.0, 2.0 * (w + h) - d);
            if d < w {
                at = vec2<f32>(d, 0.0);
            } else if d < w + h {
                at = vec2<f32>(w, d - w);
            } else if d < 2.0 * w + h {
                at = vec2<f32>(2.0 * w + h - d, h);
            }
            return array(at, canvas);
        }
        case 5u: {
            let i = u32(a.x) + min(u32(u.x * a.y), max(u32(a.y), 1u) - 1u);
            return array(points[i] + (vec2<f32>(u.y, rand(id, 11u)) - 0.5) * a.zw, canvas);
        }
        default: {
            return array(a.xy, a.xy);
        }
    }
}

fn hsv(c: vec3<f32>) -> vec3<f32> {
    let p = clamp(abs(fract(c.x + vec3<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0)) * 6.0 - 3.0) - 1.0, vec3<f32>(0.0), vec3<f32>(1.0));
    return c.z * mix(vec3<f32>(1.0), p, c.y);
}

fn unhsv(c: vec3<f32>) -> vec3<f32> {
    let p = mix(vec4<f32>(c.b, c.g, -1.0, 2.0 / 3.0), vec4<f32>(c.g, c.b, 0.0, -1.0 / 3.0), select(0.0, 1.0, c.g >= c.b));
    let q = mix(vec4<f32>(p.x, p.y, p.w, c.r), vec4<f32>(c.r, p.y, p.z, p.x), select(0.0, 1.0, c.r >= p.x));
    let d = q.x - min(q.w, q.y);
    return vec3<f32>(abs(q.z + (q.w - q.y) / (6.0 * d + 1e-10)), d / (q.x + 1e-10), q.x);
}

fn tone(id: u32) -> vec4<f32> {
    let i = sim.init;
    let t = sim.tones;
    switch u32(i[9].w) {
        case 1u: {
            let steps = t[1].z;
            var u = rand(id, KEY + 1u);
            if steps >= 1.0 {
                u = floor(u * steps) / max(steps - 1.0, 1.0);
            }
            let hue = lerp(t[0].x, t[0].y, min(u, 1.0));
            let c = hsv(vec3<f32>(hue / 360.0, lerp(t[0].z, t[0].w, rand(id, KEY + 2u)), lerp(t[1].x, t[1].y, rand(id, KEY + 3u))));
            return vec4<f32>(c, 1.0);
        }
        case 2u: {
            let n = max(t[1].w, 1.0);
            let c = sim.colors[min(u32(rand(id, KEY + 4u) * n), u32(n) - 1u)];
            if all(t[2] == vec4<f32>(0.0)) {
                return c;
            }
            var v = unhsv(c.rgb);
            v.x += (rand(id, KEY + 5u) - 0.5) * 2.0 * t[2].x * 0.5;
            v.y = clamp(v.y + (rand(id, KEY + 6u) - 0.5) * 2.0 * t[2].y, 0.0, 1.0);
            v.z = clamp(v.z + (rand(id, KEY + 7u) - 0.5) * 2.0 * t[2].z, 0.0, 1.0);
            return vec4<f32>(hsv(v), c.a);
        }
        case 3u: {
            let n = u32(clamp(t[1].w, 1.0, 8.0));
            let u = bend(rand(id, 9u), i[8].z);
            var c = sim.colors[0];
            for (var k = 1u; k < n; k++) {
                let a = sim.tones[(k - 1u) / 4u * 2u][(k - 1u) % 4u];
                let b = sim.tones[k / 4u * 2u][k % 4u];
                if u >= a {
                    c = mix(sim.colors[k - 1u], sim.colors[k], min((u - a) / max(b - a, 1e-6), 1.0));
                }
            }
            return c;
        }
        default: {
            return mix(sim.colors[0], sim.colors[1], bend(rand(id, 9u), i[8].z));
        }
    }
}
