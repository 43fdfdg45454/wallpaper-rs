struct Cord {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) @interpolate(flat) textured: f32,
};

fn toned(life: f32, c: vec4<f32>) -> vec4<f32> {
    let x = life * 63.0;
    let i = min(u32(x), 62u);
    let tone = mix(look.colors[i], look.colors[i + 1u], x - f32(i));
    let base = select(vec4<f32>(decode(c.rgb), c.a), c, look.tint.y > 0.5);
    return base * tone * vec4<f32>(vec3<f32>(look.shade.x), look.tint.x);
}

fn placed(at: vec2<f32>) -> vec4<f32> {
    let p = vec3<f32>(at, 1.0);
    return vec4<f32>(dot(look.m0.xyz, p), dot(look.m1.xyz, p), 0.0, 1.0);
}

@vertex
fn vs_rope(
    @builtin(vertex_index) v: u32,
    @location(0) a: vec4<f32>,
    @location(1) ai: vec4<f32>,
    @location(2) ac: vec4<f32>,
    @location(3) b: vec4<f32>,
    @location(4) bi: vec4<f32>,
    @location(5) bc: vec4<f32>,
) -> Cord {
    var out: Cord;
    out.position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    if ai.w < 0.5 || bi.w < 0.5 {
        return out;
    }
    let far = (v & 1u) == 1u;
    let side = f32(v >> 1u) - 0.5;
    let p = select(a, b, far);
    let i = select(ai, bi, far);
    let width = look.cord.x * curve(i.z) * i.y;
    out.position = placed(p.xy + p.zw * side * width);
    out.color = toned(i.z, select(ac, bc, far));
    out.uv = vec2<f32>(i.x * look.cord.y + look.cord.z, side + 0.5);
    out.textured = look.cord.w;
    return out;
}

fn catmull(p0: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>, p3: vec2<f32>, t: f32) -> array<vec2<f32>, 2> {
    let b = p2 - p0;
    let c = 2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3;
    let d = 3.0 * p1 - p0 - 3.0 * p2 + p3;
    return array(0.5 * (2.0 * p1 + b * t + c * t * t + d * t * t * t), 0.5 * (b + c * (2.0 * t) + d * (3.0 * t * t)));
}

@vertex
fn vs_long(
    @builtin(vertex_index) v: u32,
    @builtin(instance_index) n: u32,
    @location(0) p: vec2<f32>,
    @location(1) sp: vec2<f32>,
    @location(3) a: vec2<f32>,
    @location(4) sa: vec2<f32>,
    @location(5) ca: vec4<f32>,
    @location(6) b: vec2<f32>,
    @location(7) sb: vec2<f32>,
    @location(8) cb: vec4<f32>,
    @location(9) q: vec2<f32>,
    @location(10) sq: vec2<f32>,
) -> Cord {
    var out: Cord;
    out.position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    let k = max(u32(look.extra.z), 1u);
    let j = n % k;
    if f32(j + 1u) >= look.extra.y || sa.x <= 0.0 || sb.x <= 0.0 {
        return out;
    }
    let p0 = select(a, p, j > 0u && sp.x > 0.0);
    let p3 = select(b, q, f32(j + 2u) < look.extra.y && sq.x > 0.0);
    let t = f32(v >> 1u) / (look.strand.x + 1.0);
    let c = catmull(p0, a, b, p3, t);
    let l = length(c[1]);
    if l <= 1e-4 {
        return out;
    }
    let side = f32(v & 1u) - 0.5;
    let s = mix(sa, sb, t);
    let width = s.x * curve(s.y) * look.tint.z * look.extra.w;
    out.position = placed(c[0] + vec2<f32>(-c[1].y, c[1].x) / l * side * width);
    let fade = 1.0 - (f32(j) + t) / max(look.extra.y - 1.0, 1.0);
    out.color = toned(s.y, mix(ca, cb, t)) * vec4<f32>(1.0, 1.0, 1.0, fade);
    out.uv = vec2<f32>((f32(j) + t) / max(look.extra.y - 1.0, 1.0) * look.strand.y + look.strand.z, side + 0.5);
    out.textured = look.strand.w;
    return out;
}

@fragment
fn fs_cord(in: Cord) -> @location(0) vec4<f32> {
    var s = textureSample(sprite, samp, vec2<f32>(fract(in.uv.x), in.uv.y));
    if look.tint.y > 0.5 {
        s = vec4<f32>(encode(s.rgb), s.a);
    }
    let soft = vec4<f32>(1.0, 1.0, 1.0, 1.0 - smoothstep(0.6, 1.0, abs(in.uv.y * 2.0 - 1.0)));
    let c = select(soft, s, in.textured > 0.5 && look.tint.w > 0.5) * in.color;
    return vec4<f32>(c.rgb * c.a, c.a);
}
