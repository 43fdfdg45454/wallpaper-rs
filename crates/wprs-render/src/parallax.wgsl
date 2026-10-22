struct Parallax { offset: vec2<f32>, scale: vec2<f32>, background: vec4<f32>, flags: vec4<u32>, shift: vec2<f32>, focus: f32, steps: f32 };
struct VertexOut { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };

@group(0) @binding(0) var<uniform> p: Parallax;
@group(0) @binding(1) var image: texture_2d<f32>;
@group(0) @binding(2) var depth: texture_2d<f32>;
@group(0) @binding(3) var smp: sampler;

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VertexOut {
    let q = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return VertexOut(vec4<f32>(q.x * 2.0 - 1.0, 1.0 - q.y * 2.0, 0.0, 1.0), q);
}

fn surface(uv: vec2<f32>) -> vec2<f32> {
    var prev = uv - p.shift * (1.0 - p.focus);
    var last = textureSampleLevel(depth, smp, prev, 0.0).r - 1.0;
    if last >= 0.0 { return prev; }
    for (var i = 1.0; i <= p.steps; i += 1.0) {
        let h = 1.0 - i / p.steps;
        let q = uv - p.shift * (h - p.focus);
        let d = textureSampleLevel(depth, smp, q, 0.0).r - h;
        if d >= 0.0 { return mix(q, prev, d / (d - last)); }
        prev = q;
        last = d;
    }
    return prev;
}

@fragment
fn fs_parallax(in: VertexOut) -> @location(0) vec4<f32> {
    let uv = p.offset + in.uv * p.scale;
    if any(uv < vec2(0.0)) || any(uv > vec2(1.0)) { return p.background; }
    return vec4(textureSampleLevel(image, smp, surface(uv), 0.0).rgb, 1.0);
}
