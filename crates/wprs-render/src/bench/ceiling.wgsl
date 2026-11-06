struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(1) var src: texture_2d<f32>;
@group(0) @binding(2) var smp: sampler;

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> Out {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var o: Out;
    o.pos = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2<f32>(p.x, 1.0 - p.y);
    return o;
}

@fragment
fn fs_main(o: Out) -> @location(0) vec4<f32> {
    var c = vec4<REAL>(textureSample(src, smp, o.uv));
    let a = vec4<REAL>(REAL(0.999) - REAL(o.uv.x) * REAL(0.0001));
    let b = vec4<REAL>(REAL(0.0007));
    for (var i = 0u; i < ROUNDSu; i++) {
        c = fma(c, a, b);
    }
    return vec4<f32>(c);
}
