fn soft(d: f32, w: f32) -> f32 {
    return smoothstep(w, -w, d);
}

fn field_at(uv: vec2<f32>) -> vec2<f32> {
    let size = vec2<i32>(textureDimensions(src_tex));
    let p = uv * vec2<f32>(size) - 0.5;
    let t = p - floor(p);
    let base = vec2<i32>(floor(p));
    var v: array<vec2<f32>, 4>;
    for (var k = 0; k < 4; k++) {
        let f = textureLoad(src_tex, clamp(base + vec2<i32>(k & 1, k >> 1u), vec2<i32>(0), size - 1), 0);
        v[k] = vec2<f32>(f.r, ((f.g * 65280.0 + f.b * 255.0) / 65535.0 * 2.0 - 1.0) * item.style.w);
    }
    return mix(mix(v[0], v[1], t.x), mix(v[2], v[3], t.x), t.y);
}

@fragment
fn fs_text(in: Out) -> @location(0) vec4<f32> {
    return finish(lettered(in), in);
}

fn lettered(in: Out) -> vec4<f32> {
    let size = vec2<f32>(textureDimensions(src_tex));
    let uv = item.uv.xy + in.uv * item.uv.zw;
    let px = max(length(fwidth(uv * size)) * 0.7071, 1e-4);
    let f = field_at(uv);
    let dropped = field_at(uv - item.grad.xy).y;
    let w = item.style.x;
    let blur = max(item.style.y, px * 0.5);
    let fill = select(f.x, soft(f.y, blur), item.style.z > 0.5 || item.style.y > 0.0);
    let ring = select(0.0, soft(f.y - w, blur), w > 0.0);
    let shadow = select(0.0, soft(dropped - w, max(item.grad.z, px * 0.5)), item.grad.w > 0.5);
    let c = item.color * fill + item.stroke * max(ring - fill, 0.0);
    return c + item.stop_colors[0] * shadow * (1.0 - c.a);
}
