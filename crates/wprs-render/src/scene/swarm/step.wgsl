@compute @workgroup_size(64)
fn simulate(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i == 0u {
        atomicStore(&state.head[1], 0);
        atomicStore(&state.head[3], 0);
    }
    if i >= sim.counts.x {
        return;
    }
    var p = particles[i];
    if p.age >= p.life {
        return;
    }
    let prev = p.pos;
    let dt = sim.clock.x;
    if sim.mode.x == 3u {
        follow(&p);
    }
    let t = clamp(p.age / p.life, 0.0, 1.0);
    var acc = vec2<f32>(0.0);
    var drag = 0.0;
    var slow = 1.0;
    var damp = 0.0;
    var shift = vec2<f32>(0.0);
    var look = vec2<f32>(1.0);
    let forces = min(sim.counts.z, 16u);
    for (var k = 0u; k < forces && p.age < p.life; k++) {
        let f = sim.forces[k * 4u];
        let a = sim.forces[k * 4u + 1u];
        let b = sim.forces[k * 4u + 2u];
        let w = window(sim.forces[k * 4u + 3u], t);
        if w <= 0.0 {
            continue;
        }
        let d = f.yz - p.pos;
        let r = max(sqrt(dot(d, d)), 1e-3);
        let n = d / r;
        var add = vec2<f32>(0.0);
        switch u32(f.x) {
            case 1u: {
                add = f.yz;
            }
            case 2u: {
                drag += f.y * w;
            }
            case 3u: {
                let c = curl(vec2<f32>((p.pos.x - a.x * sim.clock.y) * f.z, p.pos.y * f.z), sim.clock.y * a.y);
                add = c * f.y * a.zw;
            }
            case 4u: {
                add = n * f.w * reach(r, a);
            }
            case 5u: {
                var m = f.w * reach(r, a);
                if a.w >= 0.0 {
                    m *= 1.0 - up(a.z, a.w, r);
                }
                let pull = select(0.0, b.z * clamp((r - b.x) / max(b.y, 1.0), -1.0, 1.0), b.w > 0.0);
                add = vec2<f32>(-n.y * m + n.x * pull, n.x * m + n.y * pull);
            }
            case 6u: {
                let k2 = f.w * reach(r, a);
                let v2 = dot(p.vel, p.vel);
                add = vec2<f32>(n.x * v2 / r - n.y * k2 - p.vel.x, n.y * v2 / r + n.x * k2 - p.vel.y);
            }
            case 7u: {
                let j = sim.ids.z * 2u + 16u;
                add = (vec2<f32>(rand(p.id, j), rand(p.id, j + 1u)) * 2.0 - 1.0) * f.y;
            }
            case 8u: {
                let now = wobble(p, k, p.age);
                let next = wobble(p, k, p.age + dt);
                switch u32(f.y) {
                    case 0u: {
                        shift += (next.xy - now.xy) * b.zw;
                    }
                    case 1u: {
                        look.x *= clamp(1.0 - 0.5 * (next.z + next.x), 0.0, 1.0);
                    }
                    default: {
                        look.y *= max(1.0 + next.x, 0.0);
                    }
                }
            }
            case 10u: {
                p.spin += f.y * w * dt;
            }
            case 11u: {
                damp += f.y * w;
            }
            case 12u: {
                if f.w > 0.0 {
                    slow *= pow(max(lerp(a.z, 1.0, up(a.x, a.y, r)), 0.0), dt * 60.0 * w);
                }
            }
            default: {}
        }
        acc += add * w;
    }
    p.vel = (p.vel + acc * dt) * (exp(-drag * dt) * slow);
    for (var k = 0u; k < forces; k++) {
        let f = sim.forces[k * 4u];
        let w = window(sim.forces[k * 4u + 3u], t);
        let l = length(p.vel);
        if u32(f.x) == 9u && w > 0.0 && l > f.y {
            p.vel *= lerp(1.0, max(f.y, 0.0) / l, w);
        }
    }
    p.spin *= exp(-damp * dt);
    p.pos += p.vel * dt + shift;
    p.rot += p.spin * dt;
    p.age += dt;
    p.look = look;
    for (var k = 0u; k < min(sim.counts.w, 8u); k++) {
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
    if (sim.mode.y & 4u) != 0u {
        record(sim.links.z, i, p, i, prev);
    }
    if p.age >= p.life {
        state.data[u32(atomicAdd(&state.head[0], 1))] = i;
        if (sim.mode.y & 2u) != 0u {
            let n = u32(atomicAdd(&state.head[2], 1));
            if n < MAX_EVENTS {
                record(sim.links.y, n, p, i, prev);
            }
        }
    }
    particles[i] = p;
}
