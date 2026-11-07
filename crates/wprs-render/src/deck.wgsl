struct Deck { offset: vec2<f32>, scale: vec2<f32>, background: vec4<f32>, flags: vec4<u32>, shift: vec2<f32>, focus: f32, steps: f32, grade: Grade, levels: array<vec4<f32>, 2> };
struct VertexOut { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };

@group(0) @binding(0) var<uniform> p: Deck;
@group(0) @binding(1) var image: texture_2d<f32>;
@group(0) @binding(2) var layers: texture_2d_array<f32>;
@group(0) @binding(3) var smp: sampler;

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VertexOut {
    let q = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return VertexOut(vec4<f32>(q.x * 2.0 - 1.0, 1.0 - q.y * 2.0, 0.0, 1.0), q);
}

@fragment
fn fs_deck(in: VertexOut) -> @location(0) vec4<f32> {
    let uv = p.offset + in.uv * p.scale;
    if any(uv < vec2(0.0)) || any(uv > vec2(1.0)) { return vec4(graded(p.background.rgb, p.grade), 1.0); }
    if all(p.shift == vec2(0.0)) { return vec4(graded(textureSampleLevel(image, smp, uv, 0.0).rgb, p.grade), 1.0); }
    var rgb = vec3(0.0);
    for (var i = 0u; i < p.flags.w; i++) {
        let at = uv - p.shift * (p.levels[i / 4u][i % 4u] - p.focus);
        let c = textureSampleLevel(layers, smp, at, i, 0.0);
        rgb = mix(rgb, c.rgb, c.a);
    }
    return vec4(graded(rgb, p.grade), 1.0);
}
