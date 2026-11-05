@group(0) @binding(0) var<uniform> light: vec4<f32>;
@group(0) @binding(1) var src: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

struct Out {
    @builtin(position) at: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> Out {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return Out(vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0));
}

fn encoded(nits: vec3<f32>) -> vec3<f32> {
    let y = pow(clamp(nits / 10000.0, vec3(0.0), vec3(1.0)), vec3(0.1593017578125));
    return pow((0.8359375 + 18.8515625 * y) / (1.0 + 18.6875 * y), vec3(78.84375));
}

@fragment
fn fs_main(in: Out) -> @location(0) vec4<f32> {
    let c = max(textureLoad(src, vec2<i32>(in.at.xy), 0).rgb, vec3(0.0));
    let wide = vec3(
        dot(vec3(0.6274, 0.3293, 0.0433), c),
        dot(vec3(0.0691, 0.9195, 0.0114), c),
        dot(vec3(0.0164, 0.0880, 0.8956), c),
    );
    return vec4(encoded(min(wide, vec3(light.y)) * light.x), 1.0);
}
