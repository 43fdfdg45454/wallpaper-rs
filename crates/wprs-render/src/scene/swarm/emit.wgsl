fn pick(lo: f32, hi: f32, id: u32, k: u32, x: f32) -> f32 {
    return lerp(lo, hi, bend(rand(id, k), x));
}

fn spawn(e: u32, id: u32, ev: Event, hatched: bool) -> Particle {
    let i = sim.init;
    let pl = sim.emitters[e * 4u + 2u];
    let q = sim.emitters[e * 4u + 3u];
    let lin = mat2x2<f32>(pl.xy, pl.zw);
    let shape = place(e, id);
    let angle = radians(pick(i[1].x, i[1].y, id, 4u, i[6].z));
    let ahead = vec2<f32>(cos(angle), sin(angle));
    var v = ahead * pick(i[0].z, i[0].w, id, 3u, i[6].y);
    if i[9].z > 0.5 {
        v = vec2<f32>(pick(i[5].x, i[5].z, id, 14u, i[8].y), pick(i[5].y, i[5].w, id, 15u, i[8].y));
    }
    let out = shape[0] - shape[1];
    let l = length(out);
    let dir = select(ahead, out / l, l > 1e-4);
    v += dir * pick(i[4].x, i[4].y, id, 13u, i[8].x);
    let carry = pick(i[4].z, i[4].w, id, KEY, 1.0);
    var p: Particle;
    p.vel = lin * v + q.zw + i[9].xy * carry;
    p.pos = lin * shape[0] + q.xy;
    p.age = 0.0;
    p.life = pick(i[0].x, i[0].y, id, 2u, i[6].x);
    p.size = pick(i[1].z, i[1].w, id, 5u, i[6].w) * sim.hatch[1].x;
    p.rot = pick(i[2].x, i[2].y, id, 6u, i[7].x);
    p.spin = pick(i[2].z, i[2].w, id, 7u, i[7].y);
    p.frame = pick(i[3].x, i[3].y, id, 8u, i[7].z);
    p.rand = rand(id, 10u);
    p.id = id;
    p.color = tone(id);
    if i[3].z >= 0.0 {
        p.color.a = pick(i[3].z, i[3].w, id, 12u, i[7].w);
    }
    p.host = vec2<u32>(0u);
    p.look = vec2<f32>(1.0);
    if hatched {
        let bits = u32(sim.hatch[0].z);
        p.pos += ev.pos;
        if (bits & 1u) != 0u {
            p.color = vec4<f32>(ev.color.rgb, p.color.a);
        }
        if (bits & 2u) != 0u {
            p.size = ev.size * sim.hatch[1].x;
        }
        if (bits & 4u) != 0u {
            p.color.a = ev.color.a;
        }
        if (bits & 8u) != 0u {
            p.vel += ev.vel;
        }
        p.host = vec2<u32>(ev.slot, ev.id);
    }
    return p;
}

fn take(p: Particle) {
    let old = atomicSub(&state.head[0], 1);
    if old <= i32(sim.counts.x - sim.ids.w) {
        atomicAdd(&state.head[0], 1);
        return;
    }
    let slot = state.data[u32(old - 1)];
    particles[slot] = p;
    if (sim.mode.y & 1u) != 0u {
        let n = u32(atomicAdd(&state.head[1], 1));
        if n < MAX_EVENTS {
            record(sim.links.x, n, p, slot, p.pos);
        }
    }
}

fn hatch(ev: Event) {
    if rand(ev.id, KEY + 16u + u32(sim.hatch[0].y)) >= sim.hatch[0].x {
        return;
    }
    var j = 0u;
    let n = clamp(sim.counts.y, 1u, 8u);
    for (var e = 0u; e < n; e++) {
        var count = births(e) - select(0u, births(max(e, 1u) - 1u), e > 0u);
        if sim.mode.x == 3u {
            let r = rand(ev.id ^ pcg(sim.ids.z), KEY + 32u + e);
            count = u32(sim.rates[e / 4u][e % 4u] * sim.clock.x + r);
        }
        for (var b = 0u; b < min(count, MAX_HATCH); b++) {
            if atomicLoad(&state.head[0]) <= i32(sim.counts.x - sim.ids.w) {
                return;
            }
            if u32(atomicAdd(&state.head[3], 1)) >= sim.mode.z {
                return;
            }
            let id = pcg(ev.id ^ pcg(j ^ pcg(sim.ids.z)));
            j++;
            take(spawn(e, id, ev, true));
        }
    }
}

@compute @workgroup_size(64)
fn emit(@builtin(global_invocation_id) gid: vec3<u32>) {
    let k = gid.x;
    switch sim.mode.x {
        case 0u: {
            if k >= births(7u) {
                return;
            }
            var e = 0u;
            while e < 7u && k >= births(e) {
                e++;
            }
            var none: Event;
            take(spawn(e, sim.ids.y + k, none, false));
        }
        case 1u, 2u: {
            if k < min(host[sim.mode.x], MAX_EVENTS) {
                hatch(event(k));
            }
        }
        default: {
            if k < sim.mode.w {
                let ev = event(k);
                if ev.alive != 0u {
                    hatch(ev);
                }
            }
        }
    }
}

@compute @workgroup_size(1)
fn settle() {
    atomicStore(&state.head[2], 0);
}
