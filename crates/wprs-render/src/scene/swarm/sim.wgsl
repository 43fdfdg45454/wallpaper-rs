struct Sim {
    clock: vec4<f32>,
    ids: vec4<u32>,
    counts: vec4<u32>,
    mode: vec4<u32>,
    links: vec4<u32>,
    births: array<vec4<u32>, 2>,
    shapes: array<vec4<u32>, 2>,
    rates: array<vec4<f32>, 2>,
    emitters: array<vec4<f32>, 32>,
    init: array<vec4<f32>, 10>,
    colors: array<vec4<f32>, 10>,
    tones: array<vec4<f32>, 3>,
    forces: array<vec4<f32>, 64>,
    colliders: array<vec4<f32>, 16>,
    hatch: array<vec4<f32>, 2>,
    trail: vec4<u32>,
    orders: array<vec4<u32>, 2>,
    paths: array<vec4<f32>, 16>,
    fields: array<vec4<f32>, 4>,
    depth: array<vec4<f32>, 3>,
    remap: vec4<u32>,
    remaps: array<vec4<f32>, 24>,
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
    host: vec2<u32>,
    look: vec2<f32>,
    depth: vec2<f32>,
    spare: vec2<f32>,
};

struct Event {
    pos: vec2<f32>,
    vel: vec2<f32>,
    color: vec4<f32>,
    size: f32,
    id: u32,
    slot: u32,
    prev: vec2<f32>,
    alive: u32,
};

struct State {
    head: array<atomic<i32>, 4>,
    data: array<u32>,
};

@group(0) @binding(0) var<storage, read_write> particles: array<Particle>;
@group(0) @binding(1) var<storage, read_write> state: State;
@group(0) @binding(2) var<storage, read> points: array<vec2<f32>>;
@group(0) @binding(3) var<storage, read> host: array<u32>;
@group(0) @binding(4) var<uniform> sim: Sim;

const TAU: f32 = 6.2831853;
const KEY: u32 = 0x80000000u;
const MAX_EVENTS: u32 = 1024u;
const MAX_HATCH: u32 = 1000u;

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

fn bend(u: f32, x: f32) -> f32 {
    if x == 1.0 {
        return u;
    }
    return pow(u, x);
}

fn births(e: u32) -> u32 {
    return sim.births[e / 4u][e % 4u];
}
