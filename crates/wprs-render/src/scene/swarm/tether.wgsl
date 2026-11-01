fn between(f: vec4<f32>, a: vec4<f32>, pos: vec2<f32>) -> vec2<f32> {
    let d = a.xy - f.yz;
    let l2 = dot(d, d);
    let along = dot(pos - f.yz, d) / l2;
    let t = select(0.0, clamp(along, 0.0, 1.0), l2 > 1e-6);
    return (f.yz + d * t - pos) * f.w;
}

fn evolve(p: ptr<function, Particle>, k: u32, w: f32) {
    let f = sim.forces[k * 4u];
    let a = sim.forces[k * 4u + 1u];
    let b = sim.forces[k * 4u + 2u];
    let bits = u32(b.w);
    let speed = length((*p).vel);
    var x = f.z;
    switch u32(f.y) {
        case 2u: {
            x = (*p).life;
        }
        case 3u: {
            x = (*p).age;
        }
        case 4u: {
            x = speed;
        }
        case 5u: {
            x = rand((*p).id, KEY + 64u + k);
        }
        case 6u: {
            x = length((*p).pos - f.zw);
        }
        default: {}
    }
    let row = vec4<f32>(f32(bits & 1u), f32((bits >> 1u) & 1u), 0.0, 0.0);
    let y = shaped(x, vec4<f32>(f.y, a.x, a.y, f.z), vec4<f32>(b.x, a.z, a.w, b.y), vec4<f32>(b.z, row.x, row.y, f.w), k + 8u);
    give(p, u32(b.x), speed, y, u32(b.y), sim.clock.x, w);
}
