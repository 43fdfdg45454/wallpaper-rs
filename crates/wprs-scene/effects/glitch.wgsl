@fragment
fn fs_glitch(in: VertexOut) -> @location(0) vec4<f32> {
    let slot = floor(frame.time / param(2u).x) % 4096.0;
    let burst = hash(vec2<f32>(slot, frame.seed)) < param(0u).x * param(2u).x;
    let on = burst || (param(1u).x > 0.5 && frame.since_beat < param(2u).x);
    let m = mask_at(in.uv) * f32(on);
    let t = floor(frame.time * param(8u).x) % 4096.0 + frame.seed;
    let row = floor(layer_uv(in.uv).y * param(3u).x);
    let block = select(0.0, hash(vec2<f32>(row, t + 7.0)) * 2.0 - 1.0, hash(vec2<f32>(row, t)) > param(4u).x);
    let line = hash(vec2<f32>(floor(in.uv.y * fx.texel.w), t + 3.0)) * 2.0 - 1.0;
    let dx = (block * px(param(5u).x).x + line * px(param(7u).x).x) * m;
    let split = vec2<f32>(px(param(6u).x).x * m, 0.0);
    let uv = in.uv + vec2<f32>(dx, 0.0);
    let r = tap(uv + split);
    let g = tap(uv);
    let b = tap(uv - split);
    return vec4<f32>(r.r, g.g, b.b, max(g.a, max(r.a, b.a)));
}
