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
};

struct Key {
    kind: u32,
    count: u32,
    size: f32,
    height: f32,
};

@group(0) @binding(0) var<storage, read> particles: array<Particle>;
@group(0) @binding(1) var<storage, read_write> sorted: array<Particle>;
@group(0) @binding(2) var<storage, read_write> bins: array<atomic<u32>, 256>;
@group(0) @binding(3) var<uniform> key: Key;

fn bin(p: Particle) -> u32 {
    let t = clamp(p.age / p.life, 0.0, 1.0);
    let s = clamp(p.size * p.look.y / max(key.size, 1e-6), 0.0, 1.0);
    var k = array<f32, 5>(1.0 - t, t, s, 1.0 - s, clamp(p.pos.y / max(key.height, 1.0), 0.0, 1.0));
    return min(u32(k[min(key.kind, 4u)] * 256.0), 255u);
}

@compute @workgroup_size(64)
fn tally(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= key.count || particles[gid.x].age >= particles[gid.x].life {
        return;
    }
    atomicAdd(&bins[bin(particles[gid.x])], 1u);
}

@compute @workgroup_size(1)
fn scan() {
    var sum = 0u;
    for (var b = 0u; b < 256u; b++) {
        let n = atomicLoad(&bins[b]);
        atomicStore(&bins[b], sum);
        sum += n;
    }
}

@compute @workgroup_size(64)
fn scatter(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= key.count || particles[gid.x].age >= particles[gid.x].life {
        return;
    }
    let p = particles[gid.x];
    sorted[atomicAdd(&bins[bin(p)], 1u)] = p;
}
