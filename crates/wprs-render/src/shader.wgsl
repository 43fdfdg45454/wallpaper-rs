struct Params {
    offset: vec2<f32>,
    scale: vec2<f32>,
    background: vec4<f32>,
    flags: vec4<u32>,
    shift: vec2<f32>,
    focus: f32,
    steps: f32,
    grade: Grade,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var image: texture_2d<f32>;
@group(0) @binding(2) var image_sampler: sampler;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOut {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    var out: VertexOut;
    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let background = vec4(graded(params.background.rgb, params.grade), 1.0);
    if params.flags.y == 0u {
        return background;
    }
    let uv = params.offset + in.uv * params.scale;
    let repeat = params.flags.x == 1u;
    let outside = any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0));
    let color = textureSample(image, image_sampler, uv);
    if outside && !repeat {
        return background;
    }
    let alpha = select(color.a, 1.0, params.flags.z == 1u);
    let rgb = mix(params.background.rgb, color.rgb, alpha);
    return vec4<f32>(graded(rgb, params.grade), 1.0);
}
