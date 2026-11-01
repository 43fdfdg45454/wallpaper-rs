struct Words {
    head: array<i32, 4>,
    data: array<u32>,
};

@group(0) @binding(0) var<storage, read> particles: array<Particle>;
@group(0) @binding(1) var<storage, read> words: Words;
@group(0) @binding(2) var<storage, read_write> strands: array<u32>;
@group(0) @binding(3) var<uniform> sim: Sim;

const POINTS: u32 = 4097u;
const SCRATCH: u32 = 4096u;

var<workgroup> sums: array<u32, 256>;

struct Knot {
    pos: vec2<f32>,
    size: f32,
    life: f32,
    color: vec4<f32>,
};

fn entry(next: u32, k: u32) -> u32 {
    let id = next - 1u - k;
    let slot = words.data[sim.rope.y + id % sim.rope.x];
    let p = particles[slot];
    return select(0xffffffffu, slot, p.id == id && p.age < p.life);
}

fn knot(slot: u32) -> Knot {
    let p = particles[slot];
    let deep = sim.depth[2];
    var pos = p.pos;
    var k = 1.0;
    if deep.w > 0.5 {
        let f = deep.x + p.depth.x;
        k = select(0.0, deep.x / f, f > 1.0);
        pos = deep.yz + (p.pos - deep.yz) * k;
    }
    return Knot(pos, p.look.y * k, min(p.age / p.life, 1.0), vec4<f32>(p.color.rgb, p.color.a * p.look.x));
}

fn at(i: i32, n: u32) -> Knot {
    return knot(strands[u32(clamp(i, 0, i32(n) - 1))]);
}

@compute @workgroup_size(256)
fn twine(@builtin(local_invocation_index) t: u32) {
    let r = sim.rope.x;
    if r == 0u {
        for (var q = t; q < POINTS * 12u; q += 256u) {
            strands[SCRATCH + q] = 0u;
        }
        return;
    }
    let next = sim.ids.y + sim.births[1].w;
    let per = (r + 255u) / 256u;
    let end = min((t + 1u) * per, r);
    var count = 0u;
    for (var k = t * per; k < end; k++) {
        count += select(0u, 1u, entry(next, k) != 0xffffffffu);
    }
    sums[t] = count;
    workgroupBarrier();
    for (var o = 1u; o < 256u; o = o << 1u) {
        let v = select(0u, sums[t - min(o, t)], t >= o);
        workgroupBarrier();
        sums[t] += v;
        workgroupBarrier();
    }
    var w = sums[t] - count;
    for (var k = t * per; k < end; k++) {
        let slot = entry(next, k);
        if slot != 0xffffffffu {
            strands[w] = slot;
            w++;
        }
    }
    let total = sums[255];
    storageBarrier();
    workgroupBarrier();
    let sub = sim.rope.z & 255u;
    let curved = ((sim.rope.z >> 8u) & 1u) != 0u;
    let span = sub + 1u;
    let n = min(total, sim.rope.w / span + 1u);
    let points = select(0u, (n - 1u) * span + 1u, n >= 2u);
    for (var q = t; q < POINTS; q += 256u) {
        let o = SCRATCH + q * 12u;
        if q >= points {
            for (var c = 0u; c < 12u; c++) {
                strands[o + c] = 0u;
            }
            continue;
        }
        let seg = min(q / span, n - 2u);
        let tt = f32(q - seg * span) / f32(span);
        let s0 = i32(seg);
        let k0 = at(s0 - 1, n);
        let k1 = at(s0, n);
        let k2 = at(s0 + 1, n);
        let k3 = at(s0 + 2, n);
        var pos = k1.pos + (k2.pos - k1.pos) * tt;
        var tan = k2.pos - k1.pos;
        if curved {
            let t2 = tt * tt;
            let t3 = tt * tt * tt;
            let b = k2.pos - k0.pos;
            let c = 2.0 * k0.pos - 5.0 * k1.pos + 4.0 * k2.pos - k3.pos;
            let d = 3.0 * k1.pos - k0.pos - 3.0 * k2.pos + k3.pos;
            pos = 0.5 * (2.0 * k1.pos + b * tt + c * t2 + d * t3);
            tan = 0.5 * (b + c * (2.0 * tt) + d * (3.0 * t2));
        }
        let l = sqrt(dot(tan, tan));
        let normal = select(vec2<f32>(0.0, 1.0), vec2<f32>(-tan.y, tan.x) / l, l > 1e-6);
        let color = k1.color + (k2.color - k1.color) * tt;
        let size = k1.size + (k2.size - k1.size) * tt;
        let life = k1.life + (k2.life - k1.life) * tt;
        let out = array<f32, 12>(pos.x, pos.y, normal.x, normal.y, f32(seg) + tt, size, life, 1.0, color.r, color.g, color.b, color.a);
        for (var c = 0u; c < 12u; c++) {
            strands[o + c] = bitcast<u32>(out[c]);
        }
    }
}
