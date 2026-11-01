fn soft(d: f32, w: f32) -> f32 {
    return smoothstep(w, -w, d);
}

@fragment
fn fs_text(in: Out) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(src_tex));
    let uv = item.uv.xy + in.uv * item.uv.zw;
    let px = max(length(fwidth(uv * size)) * 0.7071, 1e-4);
    let f = textureSample(src_tex, samp, uv);
    let dropped = textureSample(src_tex, samp, uv - item.grad.xy).g;
    let w = item.style.x;
    let blur = max(item.style.y, px * 0.5);
    let fill = select(f.r, soft(f.g, blur), item.style.z > 0.5 || item.style.y > 0.0);
    let ring = select(0.0, soft(f.g - w, blur), w > 0.0);
    let shadow = select(0.0, soft(dropped - w, max(item.grad.z, px * 0.5)), item.grad.w > 0.5);
    let c = item.color * fill + item.stroke * max(ring - fill, 0.0);
    return finish(c + item.stop_colors[0] * shadow * (1.0 - c.a), in);
}
