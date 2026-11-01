struct Mold { size: vec4<f32> };
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) color: vec4<f32> };

@group(0) @binding(0) var<uniform> mold: Mold;
@group(0) @binding(1) var skin: texture_2d<f32>;
@group(0) @binding(2) var smp: sampler;

@vertex
fn vs_mold(@location(0) at: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) color: vec4<f32>) -> Out {
    let p = at / mold.size.xy;
    let lit = select(pow((color.rgb + 0.055) / 1.055, vec3(2.4)), color.rgb / 12.92, color.rgb <= vec3(0.04045));
    return Out(vec4(p.x * 2.0 - 1.0, 1.0 - p.y * 2.0, 0.0, 1.0), uv, vec4(lit, color.a));
}

@fragment
fn fs_mold(in: Out) -> @location(0) vec4<f32> {
    let c = textureSample(skin, smp, in.uv) * in.color;
    return vec4(c.rgb * c.a, c.a);
}
