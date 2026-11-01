fn field(a: vec4<f32>, pos: vec2<f32>, canvas: vec2<f32>) -> vec4<f32> {
    let size = a.zw;
    let f = pos / canvas * size - 0.5;
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
    let gx = lerp(d10 - d00, d11 - d01, uv.y) * size.x / canvas.x;
    let gy = lerp(d01 - d00, d11 - d10, uv.x) * size.y / canvas.y;
    return vec4<f32>(d, gx, gy, 1.0);
}

fn hit(p: ptr<function, Particle>, n: vec2<f32>, depth: f32, r: vec4<f32>) {
    if depth <= 0.0 {
        return;
    }
    if r.z > 0.5 {
        (*p).age = (*p).life;
        return;
    }
    (*p).pos += n * depth;
    if r.w > 0.5 {
        (*p).vel = vec2<f32>(0.0);
        (*p).spin = 0.0;
        (*p).spare.x = 1.0;
        return;
    }
    let vn = dot((*p).vel, n);
    if vn < 0.0 {
        (*p).vel = ((*p).vel - n * vn) * (1.0 - r.y) - n * vn * r.x;
    }
}

fn collide(p: ptr<function, Particle>, prev: vec2<f32>) {
    for (var k = 0u; k < min(sim.counts.w, 8u); k++) {
        let a = sim.colliders[k * 4u];
        let b = sim.colliders[k * 4u + 1u];
        let c = sim.colliders[k * 4u + 2u];
        let r = sim.colliders[k * 4u + 3u];
        switch u32(a.x) {
            case 1u: {
                hit(p, vec2<f32>(1.0, 0.0), -(*p).pos.x, r);
                hit(p, vec2<f32>(-1.0, 0.0), (*p).pos.x - sim.clock.z, r);
                hit(p, vec2<f32>(0.0, 1.0), -(*p).pos.y, r);
                hit(p, vec2<f32>(0.0, -1.0), (*p).pos.y - sim.clock.w, r);
            }
            case 2u: {
                hit(p, vec2<f32>(a.w, b.x), -dot((*p).pos - a.yz, vec2<f32>(a.w, b.x)), r);
            }
            case 3u: {
                let d = (*p).pos - a.yz;
                let l = length(d);
                let n = select(vec2<f32>(0.0, -1.0), d / l, l > 1e-6);
                if b.x > 0.5 {
                    hit(p, -n, l - a.w, r);
                } else {
                    hit(p, n, a.w - l, r);
                }
            }
            case 4u: {
                let g = field(a, (*p).pos, sim.clock.zw);
                if g.w > 0.5 {
                    let l = length(g.yz);
                    hit(p, select(vec2<f32>(0.0, -1.0), g.yz / l, l > 1e-6), -g.x, r);
                }
            }
            case 5u: {
                boxed(p, a, b, r);
            }
            case 6u: {
                crossed(p, a, b, prev, r);
            }
            case 7u: {
                let q = mat2x2<f32>(b.xy, b.zw) * (*p).pos + c.xy;
                let g = field(a, q, c.zw);
                if g.w > 0.5 {
                    let n = vec2<f32>(dot(b.xy, g.yz), dot(b.zw, g.yz));
                    let l = length(n);
                    let unit = 1.0 / max(sqrt(abs(b.x * b.w - b.y * b.z)), 1e-6);
                    hit(p, select(vec2<f32>(0.0, -1.0), n / l, l > 1e-6), -g.x * unit, r);
                }
            }
            default: {}
        }
    }
}
