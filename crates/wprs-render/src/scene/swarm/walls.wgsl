fn boxed(p: ptr<function, Particle>, a: vec4<f32>, b: vec4<f32>, r: vec4<f32>) {
    let u = b.xy;
    let v = vec2<f32>(-b.y, b.x);
    let d = (*p).pos - a.yz;
    let q = vec2<f32>(dot(d, u), dot(d, v));
    if a.w > 0.5 {
        hit(p, -u, q.x - b.z, r);
        hit(p, u, -b.z - q.x, r);
        hit(p, -v, q.y - b.w, r);
        hit(p, v, -b.w - q.y, r);
        return;
    }
    let pen = b.zw - abs(q);
    if pen.x > 0.0 && pen.y > 0.0 {
        if pen.x < pen.y {
            hit(p, u * select(-1.0, 1.0, q.x >= 0.0), pen.x, r);
        } else {
            hit(p, v * select(-1.0, 1.0, q.y >= 0.0), pen.y, r);
        }
    }
}

fn crossed(p: ptr<function, Particle>, a: vec4<f32>, b: vec4<f32>, prev: vec2<f32>, r: vec4<f32>) {
    let o = a.yz;
    let d = b.xy - o;
    let s0 = d.x * (prev.y - o.y) - d.y * (prev.x - o.x);
    let s1 = d.x * ((*p).pos.y - o.y) - d.y * ((*p).pos.x - o.x);
    let l2 = dot(d, d);
    if s0 * s1 >= 0.0 || l2 < 1e-9 {
        return;
    }
    let t = s0 / (s0 - s1);
    let at = prev + ((*p).pos - prev) * t;
    let along = dot(at - o, d) / l2;
    if along < 0.0 || along > 1.0 {
        return;
    }
    let l = sqrt(l2);
    let k = select(1.0, -1.0, s0 < 0.0);
    let n = vec2<f32>(-d.y / l * k, d.x / l * k);
    hit(p, n, 0.01 - dot((*p).pos - o, n), r);
}
