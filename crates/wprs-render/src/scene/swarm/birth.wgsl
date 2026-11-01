fn turbulent(id: u32, v: vec2<f32>) -> vec2<f32> {
    let f = sim.fields[0];
    let g = sim.fields[1];
    if g.w < 0.5 {
        return v;
    }
    let x = f.z * sim.clock.y + g.z + f.w * rand(id, KEY + 40u) + f32(sim.ids.x & 1023u);
    let a = radians(f.x + (noise1(x) * 2.0 - 1.0) * f.y * 180.0);
    return vec2<f32>(cos(a), sin(a)) * lerp(g.x, g.y, rand(id, KEY + 41u));
}

fn shifted(id: u32, at: vec2<f32>) -> vec2<f32> {
    let f = sim.fields[2];
    let g = sim.fields[3];
    if g.w < 0.5 {
        return at;
    }
    let r = rand(id, KEY + 42u);
    let p = vec2<f32>(at.x * f.y / 100.0 + r * f.z * 17.0 + f.w * sim.clock.y, at.y * f.y / 100.0 + r * f.z * 31.0);
    let octaves = i32(g.z);
    let norm = f.x / (1.0 - pow(0.5, f32(octaves)));
    var off = vec2<f32>(fbm(p, octaves), fbm(p + vec2<f32>(5.2, 1.3), octaves)) * norm;
    off = select(off, g.xy * abs(off), g.xy != vec2<f32>(0.0));
    return at + off;
}

fn deep(d: vec4<f32>, id: u32, k: u32) -> f32 {
    return select(0.0, pick(d.x, d.y, id, k, d.z), d.w > 0.5);
}

fn remapped(x: f32, k: u32) -> f32 {
    return shaped(x, sim.remaps[k * 3u], sim.remaps[k * 3u + 1u], sim.remaps[k * 3u + 2u], k);
}

fn shaped(x: f32, a: vec4<f32>, b: vec4<f32>, c: vec4<f32>, k: u32) -> f32 {
    var t = (x - a.y) / (a.z - a.y);
    if c.y > 0.5 {
        t = clamp(t, 0.0, 1.0);
    }
    if u32(c.x) == 1u {
        t = 0.5 + 0.5 * sin(TAU * t);
    } else if u32(c.x) == 2u {
        t = noise1(t + f32(k) * 13.7);
    }
    let y = lerp(b.y, b.z, t);
    return select(y, clamp(y, min(b.y, b.z), max(b.y, b.z)), c.z > 0.5);
}

fn op(v: f32, y: f32, kind: u32) -> f32 {
    switch kind {
        case 1u: {
            return v * y;
        }
        case 2u: {
            return v + y;
        }
        case 3u: {
            return v - y;
        }
        default: {
            return y;
        }
    }
}

fn remap(p: ptr<function, Particle>) {
    for (var k = 0u; k < min(sim.remap.x, 8u); k++) {
        let a = sim.remaps[k * 3u];
        let b = sim.remaps[k * 3u + 1u];
        let speed = length((*p).vel);
        var x = a.w;
        switch u32(a.x) {
            case 1u: {
                x = sim.clock.y;
            }
            case 2u: {
                x = (*p).life;
            }
            case 3u: {
                x = (*p).age;
            }
            case 4u: {
                x = speed;
            }
            case 5u: {
                x = rand((*p).id, KEY + 56u + k);
            }
            case 6u: {
                x = distance((*p).pos, vec2<f32>(a.w, sim.remaps[k * 3u + 2u].w));
            }
            default: {}
        }
        let y = remapped(x, k);
        give(p, u32(b.x), speed, y, u32(b.w), -1.0, 1.0);
    }
}

fn applied(v: f32, y: f32, kind: u32, rate: f32, w: f32) -> f32 {
    if rate < 0.0 {
        return op(v, y, kind);
    }
    switch kind {
        case 1u: {
            return v * pow(max(y, 0.0), rate * w);
        }
        case 2u: {
            return v + y * rate * w;
        }
        case 3u: {
            return v - y * rate * w;
        }
        default: {
            return lerp(v, y, w);
        }
    }
}

fn give(p: ptr<function, Particle>, out: u32, speed: f32, y: f32, o: u32, rate: f32, w: f32) {
    switch out {
        case 0u: {
            (*p).size = max(applied((*p).size, y, o, rate, w), 0.0);
        }
        case 1u: {
            (*p).color.a = clamp(applied((*p).color.a, y, o, rate, w), 0.0, 1.0);
        }
        case 2u: {
            let c = vec3<f32>(applied((*p).color.r, y, o, rate, w), applied((*p).color.g, y, o, rate, w), applied((*p).color.b, y, o, rate, w));
            (*p).color = vec4<f32>(max(c, vec3<f32>(0.0)), (*p).color.a);
        }
        case 3u: {
            let n = applied(speed, y, o, rate, w);
            (*p).vel = select(vec2<f32>(n, 0.0), (*p).vel * n / speed, speed > 1e-6);
        }
        case 4u: {
            (*p).life = max(applied((*p).life, y, o, rate, w), 1e-3);
        }
        case 5u: {
            (*p).rot = applied((*p).rot, y, o, rate, w);
        }
        case 6u: {
            (*p).spin = applied((*p).spin, y, o, rate, w);
        }
        default: {
            (*p).frame = max(applied((*p).frame, y, o, rate, w), 0.0);
        }
    }
}
