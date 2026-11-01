fn pick(lo: f32, hi: f32, id: u32, k: u32, x: f32) -> f32 {
    return lerp(lo, hi, bend(rand(id, k), x));
}

fn nearness(z: f32) -> f32 {
    let focal = sim.hatch[1].z;
    if focal <= 0.0 {
        return 1.0;
    }
    let f = focal + z;
    return select(0.0, focal / f, f > 1.0);
}

fn spawn(e: u32, id: u32, ev: Event, hatched: bool, order: u32) -> Particle {
    let i = sim.init;
    let pl = sim.emitters[e * 4u + 2u];
    let q = sim.emitters[e * 4u + 3u];
    let lin = mat2x2<f32>(pl.xy, pl.zw);
    let shape = place(e, id, order);
    let at = shifted(id, shape[0]);
    let angle = radians(pick(i[1].x, i[1].y, id, 4u, i[6].z));
    let ahead = vec2<f32>(cos(angle), sin(angle));
    var v = ahead * pick(i[0].z, i[0].w, id, 3u, i[6].y);
    if i[9].z > 0.5 {
        v = vec2<f32>(pick(i[5].x, i[5].z, id, 14u, i[8].y), pick(i[5].y, i[5].w, id, 15u, i[8].y));
    }
    v = turbulent(id, v);
    let out = at - shape[1];
    let l = length(out);
    let dir = select(ahead, out / l, l > 1e-4);
    v += dir * pick(i[4].x, i[4].y, id, 13u, i[8].x);
    let carry = pick(i[4].z, i[4].w, id, KEY, 1.0);
    var p: Particle;
    p.vel = lin * v + q.zw + i[9].xy * carry;
    p.pos = lin * at + q.xy;
    p.age = 0.0;
    p.life = pick(i[0].x, i[0].y, id, 2u, i[6].x);
    p.size = pick(i[1].z, i[1].w, id, 5u, i[6].w) * sim.hatch[1].x * shape[2].x;
    p.rot = pick(i[2].x, i[2].y, id, 6u, i[7].x);
    p.spin = pick(i[2].z, i[2].w, id, 7u, i[7].y);
    p.frame = pick(i[3].x, i[3].y, id, 8u, i[7].z);
    p.rand = rand(id, 10u);
    p.id = id;
    p.color = tone(id);
    if i[3].z >= 0.0 {
        p.color.a = pick(i[3].z, i[3].w, id, 12u, i[7].w);
    }
    if sim.shapes[e / 4u][e % 4u] == 8u {
        let a = sim.emitters[e * 4u];
        let w = sim.paths[e * 2u];
        p.pos += (vec2<f32>(rand(id, KEY + 49u), rand(id, KEY + 50u)) - 0.5) * a.z;
        let flags = u32(a.w);
        if (flags & 1u) != 0u {
            let c = unpack4x8unorm(bitcast<u32>(shape[2].y));
            p.color = vec4<f32>(c.rgb, p.color.a * c.a);
        }
        if (flags & 2u) != 0u {
            p.vel += mat2x2<f32>(w.xy, w.zw) * shape[0] + sim.paths[e * 2u + 1u].xy;
        }
    }
    p.host = vec2<u32>(0u);
    p.look = vec2<f32>(1.0);
    p.depth = vec2<f32>(deep(sim.depth[0], id, KEY + 44u), deep(sim.depth[1], id, KEY + 45u));
    p.spare = vec2<f32>(0.0);
    if hatched {
        let bits = u32(sim.hatch[0].z);
        let near = nearness(ev.depth);
        p.pos += select(ev.pos, sim.depth[2].yz + (ev.pos - sim.depth[2].yz) * near, sim.hatch[1].z > 0.0);
        p.size *= near;
        if (bits & 1u) != 0u {
            p.color = vec4<f32>(ev.color.rgb, p.color.a);
        }
        if (bits & 2u) != 0u {
            p.size = ev.size * sim.hatch[1].x * near;
        }
        if (bits & 4u) != 0u {
            p.color.a = ev.color.a;
        }
        if (bits & 8u) != 0u {
            p.vel += ev.vel * near;
        }
        p.host = vec2<u32>(ev.slot, ev.id);
        if sim.depth[2].w > 0.5 {
            p.depth.x += ev.depth;
        }
        p.spare.y = ev.depth;
    }
    remap(&p);
    return p;
}
