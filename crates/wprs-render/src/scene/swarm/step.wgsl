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
    let before = p.age;
    let dt = sim.clock.x;
    if p.spare.x > 0.5 {
        p.age += dt;
        finish(i, p, prev, before);
        return;
    }
    if sim.mode.x == 3u {
        follow(&p);
    }
    let t = clamp(p.age / p.life, 0.0, 1.0);
    var g = Pull(vec2<f32>(0.0), vec2<f32>(0.0), vec2<f32>(1.0), 0.0, 1.0, 0.0, vec2<f32>(0.0));
    if sim.flock.y > 0u {
        let o = sim.flock.w + 2u * i;
        g.nudge = vec2<f32>(bitcast<f32>(state.data[o]), bitcast<f32>(state.data[o + 1u]));
    }
    let forces = min(sim.counts.z, 16u);
    for (var k = 0u; k < forces && p.age < p.life; k++) {
        pull(&p, k, t, dt, &g);
    }
    p.vel = (p.vel + g.acc * dt) * (exp(-g.drag * dt) * g.slow);
    for (var k = 0u; k < forces; k++) {
        let f = sim.forces[k * 4u];
        let w = window(sim.forces[k * 4u + 3u], t);
        let l = length(p.vel);
        let most = select(f.y, sim.forces[k * 4u + 1u].y, u32(f.x) == 16u);
        let capped = u32(f.x) == 9u || (u32(f.x) == 16u && most > 0.0);
        if capped && w > 0.0 && l > most {
            p.vel *= lerp(1.0, max(most, 0.0) / l, w);
        }
    }
    p.spin *= exp(-g.damp * dt);
    p.pos += p.vel * dt + g.shift;
    p.rot += p.spin * dt;
    p.age += dt;
    p.depth.x += p.depth.y * dt;
    p.look = g.look;
    collide(&p, prev);
    finish(i, p, prev, before);
}

fn finish(i: u32, q: Particle, prev: vec2<f32>, before: f32) {
    var p = q;
    if (sim.mode.y & 4u) != 0u {
        record(sim.links.z, i, p, i, prev);
    }
    if sim.trail.y > 0u {
        mark(i, p, before);
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
