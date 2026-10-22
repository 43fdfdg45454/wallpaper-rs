@fragment
fn fs_crt(in: VertexOut) -> @location(0) vec4<f32> {
    let m = mask_at(in.uv);
    let p = layer_uv(in.uv) * 2.0 - 1.0;
    let q = p * (1.0 + param(0u).x * m * dot(p, p) * 0.25);
    let at = q * 0.5 + 0.5;
    let uv = from_layer(at);
    let fade = 1.0 - smoothstep(vec2<f32>(0.99), vec2<f32>(1.0), abs(q));
    var c = tap(uv) * fade.x * fade.y;
    if param(5u).x > 0.0 {
        let o = px(2.0);
        let d = vec2<f32>(o.x, -o.y);
        let halo = tap(uv + o) + tap(uv - o) + tap(uv + d) + tap(uv - d);
        c += halo * 0.25 * param(5u).x * m;
    }
    let scan = param(2u).x * m;
    let phosphor = param(3u).x * m;
    let line = abs(sin((uv / px(1.0)).y * TAU * 0.5 / param(1u).x));
    let column = u32(in.position.x) % 3u;
    let tint = vec3<f32>(f32(column == 0u), f32(column == 1u), f32(column == 2u));
    let edge = pow(max(16.0 * at.x * at.y * (1.0 - at.x) * (1.0 - at.y), 0.0), 0.25);
    let shade = mix(1.0, line, scan) / (1.0 - 0.3634 * scan) * mix(1.0, edge, param(4u).x * m);
    let rgb = c.rgb * shade * mix(vec3<f32>(1.0), tint, phosphor) / (1.0 - 0.6667 * phosphor);
    return vec4<f32>(rgb, min(c.a, 1.0));
}
