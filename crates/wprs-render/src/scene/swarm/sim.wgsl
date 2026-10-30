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

fn record(base: u32, i: u32, p: Particle, slot: u32, prev: vec2<f32>) {
    let o = base + i * 16u;
    var words = array<u32, 16>(
        bitcast<u32>(p.pos.x), bitcast<u32>(p.pos.y), bitcast<u32>(p.vel.x), bitcast<u32>(p.vel.y),
        bitcast<u32>(p.color.x), bitcast<u32>(p.color.y), bitcast<u32>(p.color.z), bitcast<u32>(p.color.w),
        bitcast<u32>(p.size), bitcast<u32>(p.rot), p.id, slot,
        bitcast<u32>(prev.x), bitcast<u32>(prev.y), select(0u, 1u, p.age < p.life), 0u,
    );
    for (var k = 0u; k < 16u; k++) {
        state.data[o + k] = words[k];
    }
}

fn mark(i: u32, p: Particle) {
    let k = sim.trail.y;
    let o = sim.trail.x + i * k * 4u;
    for (var j = k - 1u; j > 0u; j--) {
        for (var c = 0u; c < 4u; c++) {
            state.data[o + j * 4u + c] = state.data[o + j * 4u - 4u + c];
        }
    }
    state.data[o] = bitcast<u32>(p.pos.x);
    state.data[o + 1u] = bitcast<u32>(p.pos.y);
    state.data[o + 2u] = pack2x16float(vec2<f32>(p.size * p.look.y, min(p.age / p.life, 1.0)));
    state.data[o + 3u] = pack4x8unorm(vec4<f32>(p.color.rgb, p.color.a * p.look.x));
    if p.age >= p.life {
        for (var j = 0u; j < k; j++) {
            state.data[o + j * 4u + 2u] = 0u;
        }
    }
}

fn event(i: u32) -> Event {
    let o = 4u + sim.links.w + i * 16u;
    let f = vec4<f32>(bitcast<f32>(host[o]), bitcast<f32>(host[o + 1u]), bitcast<f32>(host[o + 2u]), bitcast<f32>(host[o + 3u]));
    let c = vec4<f32>(bitcast<f32>(host[o + 4u]), bitcast<f32>(host[o + 5u]), bitcast<f32>(host[o + 6u]), bitcast<f32>(host[o + 7u]));
    let prev = vec2<f32>(bitcast<f32>(host[o + 12u]), bitcast<f32>(host[o + 13u]));
    return Event(f.xy, f.zw, c, bitcast<f32>(host[o + 8u]), host[o + 10u], host[o + 11u], prev, host[o + 14u]);
}
