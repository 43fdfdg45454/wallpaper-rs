struct Bulb {
    at: vec4<f32>,
    color: vec4<f32>,
    dir: vec4<f32>,
    more: vec4<f32>,
    air: vec4<f32>,
};

struct Lights {
    head: vec4<f32>,
    ambient: vec4<f32>,
    sky: vec4<f32>,
    bulbs: array<Bulb, 12>,
};

fn reach(b: Bulb, p: vec2<f32>) -> vec4<f32> {
    let kind = b.color.w;
    if kind > 2.5 {
        return vec4<f32>(-b.dir.xyz, 1.0);
    }
    var src = b.at.xy;
    if kind > 1.5 {
        let ab = b.more.yz * 2.0;
        let t = clamp(dot(p - (src - b.more.yz), ab) / max(dot(ab, ab), 1e-6), 0.0, 1.0);
        src = src - b.more.yz + ab * t;
    }
    let v = vec3<f32>(src - p, b.at.z);
    let d = length(v);
    if d >= b.at.w {
        return vec4<f32>(0.0);
    }
    let f = 1.0 - d * d / (b.at.w * b.at.w);
    let l = v / max(d, 1e-3);
    var k = f * f;
    if kind > 0.5 && kind < 1.5 {
        k *= smoothstep(b.dir.w, b.more.x, dot(-l, b.dir.xyz));
    }
    return vec4<f32>(l, k);
}
