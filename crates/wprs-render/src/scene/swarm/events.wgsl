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
