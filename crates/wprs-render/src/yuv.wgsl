struct Rows {
    r: vec4<f32>,
    g: vec4<f32>,
    b: vec4<f32>,
    turn: vec4<f32>,
}

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0) var<uniform> rows: Rows;
@group(0) @binding(1) var luma: texture_2d<f32>;
@group(0) @binding(2) var chroma: texture_2d<f32>;
@group(0) @binding(3) var linear: sampler;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOut {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    return VertexOut(vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0), vec2<f32>(x, y));
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let p = in.uv - vec2<f32>(0.5);
    let uv = vec2<f32>(dot(rows.turn.xy, p), dot(rows.turn.zw, p)) + vec2<f32>(0.5);
    let c = textureSample(chroma, linear, uv).rg;
    let yuv = vec4<f32>(textureSample(luma, linear, uv).r, c, 1.0);
    let rgb = clamp(vec3<f32>(dot(rows.r, yuv), dot(rows.g, yuv), dot(rows.b, yuv)), vec3<f32>(0.0), vec3<f32>(1.0));
    let low = rgb <= vec3<f32>(0.04045);
    return vec4<f32>(select(pow((rgb + 0.055) / 1.055, vec3<f32>(2.4)), rgb / 12.92, low), 1.0);
}
