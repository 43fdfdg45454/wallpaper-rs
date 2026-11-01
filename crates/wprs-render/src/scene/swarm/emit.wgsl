fn take(p: Particle) {
    let old = atomicSub(&state.head[0], 1);
    if old <= i32(sim.counts.x - sim.ids.w) {
        atomicAdd(&state.head[0], 1);
        return;
    }
    let slot = state.data[u32(old - 1)];
    particles[slot] = p;
    if sim.rope.x > 0u {
        state.data[sim.rope.y + p.id % sim.rope.x] = slot;
    }
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
            take(spawn(e, id, ev, true, j));
            j++;
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
            let order = sim.orders[e / 4u][e % 4u] + k - select(0u, births(max(e, 1u) - 1u), e > 0u);
            take(spawn(e, sim.ids.y + k, none, false, order));
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
