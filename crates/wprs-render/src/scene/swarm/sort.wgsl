struct Particle {
    pos: vec2<f32>,
    vel: vec2<f32>,
    age: f32,
    life: f32,
    size: f32,
    rot: f32,
    spin: f32,
    frame: f32,
    rand: f32,
    id: u32,
    color: vec4<f32>,
    host: vec2<u32>,
    look: vec2<f32>,
    depth: vec2<f32>,
    spare: vec2<f32>,
};

struct Key {
    kind: u32,
    count: u32,
    size: f32,
    height: f32,
};

@group(0) @binding(0) var<storage, read> particles: array<Particle>;
@group(0) @binding(1) var<storage, read_write> sorted: array<Particle>;
@group(0) @binding(2) var<storage, read_write> bins: array<atomic<u32>, 1024>;

const DEAD: u32 = 256u;
const AT: u32 = 512u;

var<workgroup> part: array<u32, 256>;
@group(0) @binding(3) var<uniform> key: Key;

fn bin(p: Particle) -> u32 {
    let t = clamp(p.age / p.life, 0.0, 1.0);
    let s = clamp(p.size * p.look.y / max(key.size, 1e-6), 0.0, 1.0);
    let far = clamp(0.5 - p.depth.x / (2.0 * max(key.size, 1e-6)), 0.0, 1.0);
    var k = array<f32, 6>(1.0 - t, t, s, 1.0 - s, clamp(p.pos.y / max(key.height, 1.0), 0.0, 1.0), far);
    return min(u32(k[min(key.kind, 5u)] * 256.0), 255u);
}

fn slot(p: Particle) -> u32 {
    return select(bin(p), DEAD, p.age >= p.life);
}

@compute @workgroup_size(64)
fn tally(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x < key.count {
        atomicAdd(&bins[slot(particles[gid.x])], 1u);
    }
}

@compute @workgroup_size(256)
fn scan(@builtin(local_invocation_index) t: u32) {
    let n = atomicExchange(&bins[t], 0u);
    part[t] = n;
    for (var d = 1u; d < 256u; d = d << 1u) {
        workgroupBarrier();
        let v = select(0u, part[t - min(d, t)], t >= d);
        workgroupBarrier();
        part[t] += v;
    }
    atomicStore(&bins[AT + t], part[t] - n);
    if t == 255u {
        atomicStore(&bins[AT + DEAD], part[t]);
        atomicStore(&bins[DEAD], 0u);
    }
}

@compute @workgroup_size(64)
fn scatter(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x < key.count {
        let p = particles[gid.x];
        sorted[atomicAdd(&bins[AT + slot(p)], 1u)] = p;
    }
}
