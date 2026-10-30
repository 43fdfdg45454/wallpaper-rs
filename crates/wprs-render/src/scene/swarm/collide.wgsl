fn field(a: vec4<f32>, pos: vec2<f32>) -> vec4<f32> {
    let size = a.zw;
    let f = pos / sim.clock.zw * size - 0.5;
    if !(f.x > -0.5 && f.y > -0.5 && f.x < size.x - 0.5 && f.y < size.y - 0.5) {
        return vec4<f32>(0.0);
    }
    let c0 = clamp(floor(f), vec2<f32>(0.0), size - 1.0);
    let c1 = min(c0 + 1.0, size - 1.0);
    let uv = clamp(f - c0, vec2<f32>(0.0), vec2<f32>(1.0));
    let start = u32(a.y);
    let d00 = points[start + u32(c0.y * size.x + c0.x)].x;
    let d10 = points[start + u32(c0.y * size.x + c1.x)].x;
    let d01 = points[start + u32(c1.y * size.x + c0.x)].x;
    let d11 = points[start + u32(c1.y * size.x + c1.x)].x;
    let d = lerp(lerp(d00, d10, uv.x), lerp(d01, d11, uv.x), uv.y);
    let gx = lerp(d10 - d00, d11 - d01, uv.y) * size.x / sim.clock.z;
    let gy = lerp(d01 - d00, d11 - d10, uv.x) * size.y / sim.clock.w;
    return vec4<f32>(d, gx, gy, 1.0);
}

fn hit(p: ptr<function, Particle>, n: vec2<f32>, depth: f32, b: vec4<f32>) {
    if depth <= 0.0 {
        return;
    }
    if b.w > 0.5 {
        (*p).age = (*p).life;
        return;
    }
    (*p).pos += n * depth;
    let vn = dot((*p).vel, n);
    if vn < 0.0 {
        (*p).vel = ((*p).vel - n * vn) * (1.0 - b.z) - n * vn * b.y;
    }
}

fn collide(p: ptr<function, Particle>) {
    for (var k = 0u; k < min(sim.counts.w, 8u); k++) {
        let a = sim.colliders[k * 2u];
        let b = sim.colliders[k * 2u + 1u];
        switch u32(a.x) {
            case 1u: {
                hit(p, vec2<f32>(1.0, 0.0), -(*p).pos.x, b);
                hit(p, vec2<f32>(-1.0, 0.0), (*p).pos.x - sim.clock.z, b);
                hit(p, vec2<f32>(0.0, 1.0), -(*p).pos.y, b);
                hit(p, vec2<f32>(0.0, -1.0), (*p).pos.y - sim.clock.w, b);
            }
            case 2u: {
                hit(p, vec2<f32>(a.w, b.x), -dot((*p).pos - a.yz, vec2<f32>(a.w, b.x)), b);
            }
            case 3u: {
                let d = (*p).pos - a.yz;
                let r = length(d);
                hit(p, select(vec2<f32>(0.0, -1.0), d / r, r > 1e-6), a.w - r, b);
            }
            case 4u: {
                let g = field(a, (*p).pos);
                if g.w > 0.5 {
                    let l = length(g.yz);
                    hit(p, select(vec2<f32>(0.0, -1.0), g.yz / l, l > 1e-6), -g.x, b);
                }
            }
            default: {}
        }
    }
}
