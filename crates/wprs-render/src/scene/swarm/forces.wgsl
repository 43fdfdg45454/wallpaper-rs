struct Pull {
    acc: vec2<f32>,
    shift: vec2<f32>,
    look: vec2<f32>,
    drag: f32,
    slow: f32,
    damp: f32,
};

fn pull(p: ptr<function, Particle>, k: u32, t: f32, dt: f32, g: ptr<function, Pull>) {
    let f = sim.forces[k * 4u];
    let a = sim.forces[k * 4u + 1u];
    let b = sim.forces[k * 4u + 2u];
    let w = window(sim.forces[k * 4u + 3u], t);
    if w <= 0.0 {
        return;
    }
    let d = f.yz - (*p).pos;
    let r = max(sqrt(dot(d, d)), 1e-3);
    let n = d / r;
    var add = vec2<f32>(0.0);
    switch u32(f.x) {
        case 1u: {
            add = f.yz;
        }
        case 2u: {
            (*g).drag += f.y * w;
        }
        case 3u: {
            let c = curl(vec2<f32>(((*p).pos.x - a.x * sim.clock.y) * f.z, (*p).pos.y * f.z), sim.clock.y * a.y);
            add = c * f.y * a.zw;
        }
        case 4u: {
            add = n * f.w * reach(r, a);
        }
        case 5u: {
            var m = f.w * reach(r, a);
            if a.w >= 0.0 {
                m *= 1.0 - up(a.z, a.w, r);
            }
            let tug = select(0.0, b.z * clamp((r - b.x) / max(b.y, 1.0), -1.0, 1.0), b.w > 0.0);
            add = vec2<f32>(-n.y * m + n.x * tug, n.x * m + n.y * tug);
        }
        case 6u: {
            let k2 = f.w * reach(r, a);
            let v2 = dot((*p).vel, (*p).vel);
            add = vec2<f32>(n.x * v2 / r - n.y * k2 - (*p).vel.x, n.y * v2 / r + n.x * k2 - (*p).vel.y);
        }
        case 7u: {
            let j = sim.ids.z * 2u + 16u;
            add = (vec2<f32>(rand((*p).id, j), rand((*p).id, j + 1u)) * 2.0 - 1.0) * f.y;
        }
        case 8u: {
            let now = wobble(*p, k, (*p).age);
            let next = wobble(*p, k, (*p).age + dt);
            switch u32(f.y) {
                case 0u: {
                    (*g).shift += (next.xy - now.xy) * b.zw;
                }
                case 1u: {
                    (*g).look.x *= clamp(1.0 - 0.5 * (next.z + next.x), 0.0, 1.0);
                }
                default: {
                    (*g).look.y *= max(1.0 + next.x, 0.0);
                }
            }
        }
        case 10u: {
            (*p).spin += f.y * w * dt;
        }
        case 11u: {
            (*g).damp += f.y * w;
        }
        case 12u: {
            if f.w > 0.0 {
                (*g).slow *= pow(max(lerp(a.z, 1.0, up(a.x, a.y, r)), 0.0), dt * 60.0 * w);
            }
        }
        default: {}
    }
    (*g).acc += add * w;
}
