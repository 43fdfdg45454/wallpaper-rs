struct Sim {
    clock: vec4<f32>,
    ids: vec4<u32>,
    births: vec4<u32>,
    counts: vec4<u32>,
    shapes: vec4<u32>,
    emitters: array<vec4<f32>, 8>,
    init: array<vec4<f32>, 4>,
    colors: array<vec4<f32>, 2>,
    forces: array<vec4<f32>, 8>,
    colliders: array<vec4<f32>, 8>,
};

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
};

@group(0) @binding(0) var<storage, read_write> particles: array<Particle>;
@group(0) @binding(1) var<storage, read_write> dead: array<u32>;
@group(0) @binding(2) var<storage, read_write> top: atomic<i32>;
@group(0) @binding(3) var<storage, read> points: array<vec2<f32>>;
@group(0) @binding(4) var<uniform> sim: Sim;

const TAU: f32 = 6.2831853;

fn pcg(v: u32) -> u32 {
    let s = v * 747796405u + 2891336453u;
    let w = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
    return (w >> 22u) ^ w;
}

fn rand(id: u32, k: u32) -> f32 {
    return f32(pcg(sim.ids.x ^ pcg(id ^ pcg(k))) >> 8u) / 16777216.0;
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    return a * (1.0 - t) + b * t;
}

fn place(e: u32, id: u32) -> vec2<f32> {
    let a = sim.emitters[e * 2u];
    let u = vec2<f32>(rand(id, 0u), rand(id, 1u));
    let w = sim.clock.z;
    let h = sim.clock.w;
    switch sim.shapes[e] {
        case 1u: {
            return vec2<f32>(lerp(a.x, a.z, u.x), lerp(a.y, a.w, u.y));
        }
        case 2u: {
            let r = sqrt(lerp(a.w * a.w, a.z * a.z, u.x));
            let t = TAU * u.y;
            return vec2<f32>(a.x + r * cos(t), a.y + r * sin(t));
        }
        case 3u: {
            return vec2<f32>(lerp(a.x, a.z, u.x), lerp(a.y, a.w, u.x));
        }
        case 4u: {
            let d = u.x * 2.0 * (w + h);
            if d < w {
                return vec2<f32>(d, 0.0);
            }
            if d < w + h {
                return vec2<f32>(w, d - w);
            }
            if d < 2.0 * w + h {
                return vec2<f32>(2.0 * w + h - d, h);
            }
            return vec2<f32>(0.0, 2.0 * (w + h) - d);
        }
        case 5u: {
            let i = u32(a.x) + min(u32(u.x * a.y), max(u32(a.y), 1u) - 1u);
            return points[i] + (vec2<f32>(u.y, rand(id, 11u)) - 0.5) * a.zw;
        }
        default: {
            return a.xy;
        }
    }
}

@compute @workgroup_size(64)
fn emit(@builtin(global_invocation_id) gid: vec3<u32>) {
    let k = gid.x;
    if k >= sim.births.w {
        return;
    }
    let old = atomicSub(&top, 1);
    if old <= i32(sim.counts.x - sim.ids.w) {
        atomicAdd(&top, 1);
        return;
    }
    var e = 0u;
    while e < 3u && k >= sim.births[e] {
        e++;
    }
    let id = sim.ids.y + k;
    let m = sim.emitters[e * 2u + 1u];
    let at = place(e, id);
    let angle = radians(lerp(sim.init[1].x, sim.init[1].y, rand(id, 4u)));
    let speed = lerp(sim.init[0].z, sim.init[0].w, rand(id, 3u));
    var p: Particle;
    p.pos = at + m.xy;
    p.vel = vec2<f32>(cos(angle) * speed + m.z, sin(angle) * speed + m.w);
    p.age = 0.0;
    p.life = lerp(sim.init[0].x, sim.init[0].y, rand(id, 2u));
    p.size = lerp(sim.init[1].z, sim.init[1].w, rand(id, 5u));
    p.rot = lerp(sim.init[2].x, sim.init[2].y, rand(id, 6u));
    p.spin = lerp(sim.init[2].z, sim.init[2].w, rand(id, 7u));
    p.frame = lerp(sim.init[3].x, sim.init[3].y, rand(id, 8u));
    p.rand = rand(id, 10u);
    p.id = id;
    p.color = mix(sim.colors[0], sim.colors[1], rand(id, 9u));
    particles[dead[old - 1]] = p;
}
