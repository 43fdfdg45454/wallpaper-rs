struct Ribbon {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) across: f32,
};

@vertex
fn vs_trail(
    @builtin(vertex_index) v: u32,
    @builtin(instance_index) n: u32,
    @location(0) a: vec2<f32>,
    @location(1) sa: vec2<f32>,
    @location(2) ca: vec4<f32>,
    @location(3) b: vec2<f32>,
    @location(4) sb: vec2<f32>,
    @location(5) cb: vec4<f32>,
) -> Ribbon {
    var out: Ribbon;
    out.position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    let k = max(u32(look.extra.z), 1u);
    let j = n % k;
    let d = b - a;
    let l = length(d);
    if f32(j + 1u) >= look.extra.y || sa.x <= 0.0 || sb.x <= 0.0 || l <= 1e-4 {
        return out;
    }
    let corner = vec2<f32>(f32(v & 1u), f32(v >> 1u));
    let far = corner.x > 0.5;
    let s = select(sa, sb, far);
    let c = select(ca, cb, far);
    let width = s.x * curve(s.y) * look.tint.z * look.extra.w;
    let p = vec3<f32>(select(a, b, far) + vec2<f32>(-d.y, d.x) / l * (corner.y - 0.5) * width, 1.0);
    out.position = vec4<f32>(dot(look.m0.xyz, p), dot(look.m1.xyz, p), 0.0, 1.0);
    let x = s.y * 63.0;
    let i = min(u32(x), 62u);
    let tone = mix(look.colors[i], look.colors[i + 1u], x - f32(i));
    let base = select(vec4<f32>(decode(c.rgb), c.a), c, look.tint.y > 0.5);
    let fade = 1.0 - (f32(j) + corner.x) / max(look.extra.y - 1.0, 1.0);
    out.color = base * tone * vec4<f32>(vec3<f32>(look.shade.x), look.tint.x * fade);
    out.across = corner.y;
    return out;
}

@fragment
fn fs_trail(in: Ribbon) -> @location(0) vec4<f32> {
    let c = in.color * vec4<f32>(1.0, 1.0, 1.0, 1.0 - smoothstep(0.6, 1.0, abs(in.across * 2.0 - 1.0)));
    return vec4<f32>(c.rgb * c.a, c.a);
}
