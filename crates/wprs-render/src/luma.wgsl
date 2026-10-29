@group(0) @binding(0) var<uniform> grid: vec4<f32>;
@group(0) @binding(1) var src: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(p.x * 2.0 - 1.0, 1.0 - p.y * 2.0, 0.0, 1.0);
}

@fragment
fn fs_luma(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    let cell = 1.0 / grid.xy;
    let origin = floor(at.xy) * cell;
    var sum = 0.0;
    for (var y = 0u; y < 8u; y++) {
        for (var x = 0u; x < 8u; x++) {
            let uv = origin + (vec2<f32>(f32(x), f32(y)) + 0.5) / 8.0 * cell;
            sum += dot(textureSampleLevel(src, samp, uv, 0.0).rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
        }
    }
    return vec4<f32>(sum / 64.0, 0.0, 0.0, 1.0);
}
