struct State {
    head: array<atomic<i32>, 4>,
    data: array<u32>,
};

@group(0) @binding(0) var<storage, read_write> particles: array<Particle>;
@group(0) @binding(1) var<storage, read_write> state: State;
@group(0) @binding(2) var<storage, read> points: array<vec2<f32>>;
@group(0) @binding(3) var<storage, read> host: array<u32>;
@group(0) @binding(4) var<uniform> sim: Sim;

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
