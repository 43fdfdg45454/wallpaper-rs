fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = p - i;
    let u = f * f * (3.0 - 2.0 * f);
    let c = vec2<i32>(i);
    var g: array<f32, 4>;
    for (var k = 0; k < 4; k++) {
        let o = vec2<i32>(k & 1, k >> 1u);
        let a = rand(bitcast<u32>(c.x + o.x), bitcast<u32>(c.y + o.y)) * TAU;
        g[k] = cos(a) * (f.x - f32(o.x)) + sin(a) * (f.y - f32(o.y));
    }
    return lerp(lerp(g[0], g[1], u.x), lerp(g[2], g[3], u.x), u.y);
}

fn curl(p: vec2<f32>, t: f32) -> vec2<f32> {
    let q = vec2<f32>(p.x, p.y + t * 0.2);
    let e = 0.01;
    let dy = noise(q + vec2<f32>(0.0, e)) - noise(q - vec2<f32>(0.0, e));
    let dx = noise(q - vec2<f32>(e, 0.0)) - noise(q + vec2<f32>(e, 0.0));
    return vec2<f32>(dy, dx) / (2.0 * e);
}

fn hit(p: ptr<function, Particle>, n: vec2<f32>, depth: f32, b: vec4<f32>) {
    if depth <= 0.0 {
        return;
    }
    if b.w > 0.5 {
        (*p).age = (*p).life;
        return;
    }
    (*p).pos += n * depth;
    let vn = dot((*p).vel, n);
    if vn < 0.0 {
        (*p).vel = ((*p).vel - n * vn) * (1.0 - b.z) - n * vn * b.y;
    }
}

@compute @workgroup_size(64)
fn simulate(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i >= sim.counts.x {
        return;
    }
    var p = particles[i];
    if p.age >= p.life {
        return;
    }
    let dt = sim.clock.x;
    var acc = vec2<f32>(0.0);
    var drag = 0.0;
    for (var k = 0u; k < sim.counts.z; k++) {
        let f = sim.forces[k];
        let d = f.yz - p.pos;
        let r2 = dot(d, d);
        switch u32(f.x) {
            case 1u: {
                acc += f.yz;
            }
            case 2u: {
                drag += f.y;
            }
            case 3u: {
                acc += curl(p.pos * f.z, sim.clock.y) * f.y;
            }
            case 4u: {
                acc += d * (f.w / (max(r2, 1.0) * max(sqrt(r2), 1e-3)));
            }
            case 5u: {
                acc += vec2<f32>(-d.y, d.x) * (f.w / max(r2, 1.0));
            }
            case 6u: {
                let r = max(sqrt(r2), 1e-3);
                let n = d / r;
                let v2 = dot(p.vel, p.vel);
                acc += vec2<f32>(n.x * (v2 / r) - n.y * f.w - p.vel.x, n.y * (v2 / r) + n.x * f.w - p.vel.y);
            }
            case 7u: {
                let j = sim.ids.z * 2u + 16u;
                acc += (vec2<f32>(rand(p.id, j), rand(p.id, j + 1u)) * 2.0 - 1.0) * f.y;
            }
            default: {}
        }
    }
    p.vel = (p.vel + acc * dt) * exp(-drag * dt);
    p.pos += p.vel * dt;
    p.rot += p.spin * dt;
    p.age += dt;
    for (var k = 0u; k < sim.counts.w; k++) {
        let a = sim.colliders[k * 2u];
        let b = sim.colliders[k * 2u + 1u];
        switch u32(a.x) {
            case 1u: {
                hit(&p, vec2<f32>(1.0, 0.0), -p.pos.x, b);
                hit(&p, vec2<f32>(-1.0, 0.0), p.pos.x - sim.clock.z, b);
                hit(&p, vec2<f32>(0.0, 1.0), -p.pos.y, b);
                hit(&p, vec2<f32>(0.0, -1.0), p.pos.y - sim.clock.w, b);
            }
            case 2u: {
                hit(&p, vec2<f32>(a.w, b.x), -dot(p.pos - a.yz, vec2<f32>(a.w, b.x)), b);
            }
            case 3u: {
                let d = p.pos - a.yz;
                let r = length(d);
                hit(&p, select(vec2<f32>(0.0, -1.0), d / r, r > 1e-6), a.w - r, b);
            }
            default: {}
        }
    }
    if p.age >= p.life {
        dead[atomicAdd(&top, 1)] = i;
    }
    particles[i] = p;
}
