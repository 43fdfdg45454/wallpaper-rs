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
    colliders: array<vec4<f32>, 32>,
    hatch: array<vec4<f32>, 2>,
    trail: vec4<u32>,
    orders: array<vec4<u32>, 2>,
    paths: array<vec4<f32>, 16>,
    fields: array<vec4<f32>, 4>,
    depth: array<vec4<f32>, 3>,
    remap: vec4<u32>,
    remaps: array<vec4<f32>, 24>,
    rope: vec4<u32>,
    flock: vec4<u32>,
    boids: array<vec4<f32>, 2>,
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

const TAU: f32 = 6.2831853;
const KEY: u32 = 0x80000000u;
