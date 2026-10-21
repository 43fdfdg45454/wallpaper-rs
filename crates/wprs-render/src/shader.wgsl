// Draws an image onto the whole target according to a precomputed UV mapping.

struct Params {
    // Target position p in [0,1]^2 samples the image at offset + p * scale.
    offset: vec2<f32>,
    scale: vec2<f32>,
    // Linear-space background color used outside the image.
    background: vec4<f32>,
    // x: 1 when the image repeats, y: 1 when an image is bound.
    flags: vec4<u32>,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var image: texture_2d<f32>;
@group(0) @binding(2) var image_sampler: sampler;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// Single triangle covering the viewport; uv is (0,0) top-left, (1,1) bottom-right.
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
    if params.flags.y == 0u {
        return params.background;
    }
    let uv = params.offset + in.uv * params.scale;
    let repeat = params.flags.x == 1u;
    let outside = any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0));
    // Sample unconditionally so derivatives stay in uniform control flow.
    let color = textureSample(image, image_sampler, uv);
    if outside && !repeat {
        return params.background;
    }
    // Composite non-opaque images over the background.
    let rgb = mix(params.background.rgb, color.rgb, color.a);
    return vec4<f32>(rgb, 1.0);
}
