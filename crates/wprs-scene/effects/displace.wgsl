@fragment
fn fs_displace(in: VertexOut) -> @location(0) vec4<f32> {
    let at = (layer_uv(in.uv) * vec2<f32>(param(5u).x, 1.0) + param(4u).xy * frame.time) * param(2u).x;
    var way = vec2<f32>(gradient_noise(at * 4.0 + 3.7), gradient_noise(at * 4.0 + 19.1)) * 0.7;
    if textureDimensions(aux0).x > 1u {
        let m = textureSampleLevel(aux0, wrap, at, 0.0);
        let n = m.rgb * 2.0 - 1.0;
        way = select(n.xy / max(n.z, 0.25), (m.rg - 0.5) * 2.0, param(1u).x > 0.5) * m.a;
    }
    return tap(in.uv - way * px(param(3u).x) * mask_at(in.uv));
}
