struct Grid {
    head: array<atomic<i32>, 4>,
    data: array<atomic<u32>>,
};

@group(0) @binding(0) var<storage, read> particles: array<Particle>;
@group(0) @binding(1) var<storage, read_write> grid: Grid;
@group(0) @binding(2) var<uniform> sim: Sim;

var<workgroup> sums: array<u32, 256>;

fn cell(p: vec2<f32>, r: f32) -> vec2<i32> {
    return vec2<i32>(floor(p / r));
}

fn hashed(c: vec2<i32>) -> u32 {
    return ((bitcast<u32>(c.x) * 73856093u) ^ (bitcast<u32>(c.y) * 19349663u)) & (sim.flock.y - 1u);
}

fn bucket(i: u32) -> u32 {
    if i >= sim.counts.x {
        return 0xffffffffu;
    }
    let p = particles[i];
    return select(0xffffffffu, hashed(cell(p.pos, sim.boids[0].x)), p.age < p.life);
}

@compute @workgroup_size(64)
fn flock_clear(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x < sim.flock.y {
        atomicStore(&grid.data[sim.flock.x + gid.x], 0u);
    }
}

@compute @workgroup_size(64)
fn flock_count(@builtin(global_invocation_id) gid: vec3<u32>) {
    let h = bucket(gid.x);
    if h != 0xffffffffu {
        atomicAdd(&grid.data[sim.flock.x + h], 1u);
    }
}

@compute @workgroup_size(256)
fn flock_scan(@builtin(local_invocation_index) t: u32) {
    let g = sim.flock.x;
    let c = sim.flock.y;
    let per = (c + 255u) / 256u;
    let begin = min(t * per, c);
    let end = min(begin + per, c);
    var total = 0u;
    for (var h = begin; h < end; h++) {
        total += atomicLoad(&grid.data[g + h]);
    }
    sums[t] = total;
    workgroupBarrier();
    for (var o = 1u; o < 256u; o = o << 1u) {
        let v = select(0u, sums[t - min(o, t)], t >= o);
        workgroupBarrier();
        sums[t] += v;
        workgroupBarrier();
    }
    var at = sums[t] - total;
    for (var h = begin; h < end; h++) {
        atomicStore(&grid.data[g + c + h], at);
        atomicStore(&grid.data[g + 2u * c + h], at);
        at += atomicLoad(&grid.data[g + h]);
    }
}

@compute @workgroup_size(64)
fn flock_scatter(@builtin(global_invocation_id) gid: vec3<u32>) {
    let h = bucket(gid.x);
    if h != 0xffffffffu {
        let c = sim.flock.y;
        let at = atomicAdd(&grid.data[sim.flock.x + 2u * c + h], 1u);
        atomicStore(&grid.data[sim.flock.x + 3u * c + at], gid.x);
    }
}

@compute @workgroup_size(64)
fn flock_steer(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i >= sim.counts.x {
        return;
    }
    let p = particles[i];
    var out = vec2<f32>(0.0);
    if p.age < p.life {
        out = near(i, p);
    }
    let o = sim.flock.w + 2u * i;
    atomicStore(&grid.data[o], bitcast<u32>(out.x));
    atomicStore(&grid.data[o + 1u], bitcast<u32>(out.y));
}

fn shuffle(v: u32) -> u32 {
    let s = v * 747796405u + 2891336453u;
    let w = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
    return (w >> 22u) ^ w;
}

fn apart(a: u32, b: u32) -> vec2<f32> {
    let turn = f32(shuffle(shuffle(min(a, b)) ^ max(a, b)) >> 8u) / 16777216.0 * TAU;
    return vec2<f32>(cos(turn), sin(turn)) * select(-1.0, 1.0, a < b);
}

fn near(i: u32, p: Particle) -> vec2<f32> {
    let b = sim.boids[0];
    let r = b.x;
    let g = sim.flock.x;
    let cells = sim.flock.y;
    let c = cell(p.pos, r);
    var n = 0u;
    var at = vec2<f32>(0.0);
    var vel = vec2<f32>(0.0);
    var away = vec2<f32>(0.0);
    var seen = array<u32, 9>(0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u);
    var done = false;
    var scanned = 0u;
    for (var m = 0u; m < 9u && !done; m++) {
        let h = hashed(c + vec2<i32>(i32(m % 3u) - 1, i32(m / 3u) - 1));
        var twice = false;
        for (var q = 0u; q < m; q++) {
            twice = twice || seen[q] == h;
        }
        seen[m] = h;
        if twice {
            continue;
        }
        let start = atomicLoad(&grid.data[g + cells + h]);
        let end = start + atomicLoad(&grid.data[g + h]);
        for (var e = start; e < end; e++) {
            scanned++;
            if scanned > 4u * sim.flock.z {
                done = true;
                break;
            }
            let j = atomicLoad(&grid.data[g + 3u * cells + e]);
            let q = particles[j];
            let d = p.pos - q.pos;
            let l = sqrt(dot(d, d));
            if j == i || l >= r {
                continue;
            }
            n++;
            if l < 1e-6 {
                away += apart(p.id, q.id);
            } else {
                away += d * ((1.0 - l / r) / l);
            }
            at += q.pos;
            vel += q.vel;
            if n >= sim.flock.z {
                done = true;
                break;
            }
        }
    }
    if n == 0u {
        return vec2<f32>(0.0);
    }
    let k = f32(n);
    return away / k * b.y * r + (vel / k - p.vel) * b.z + (at / k - p.pos) * b.w;
}
