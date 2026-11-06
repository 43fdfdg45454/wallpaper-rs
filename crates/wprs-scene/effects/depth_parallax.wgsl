fn depth_parallax_aim() -> vec2<f32> {
    let t = frame.time * TAU;
    let orbit = vec2<f32>(sin(t / 17.0), 0.7 * sin(t / 23.0 + 0.6)) * param(6u).x;
    let aim = mix(orbit, (frame.cursor - 0.5) * 2.0, param(5u).x * frame.presence);
    return aim / max(length(aim), 1.0);
}

@fragment
fn fs_depth_parallax(in: VertexOut) -> @location(0) vec4<f32> {
    let aim = depth_parallax_aim();
    if textureDimensions(aux0).x <= 1u || (dot(aim, aim) * param(1u).x < 1e-6 && param(4u).x <= 1.0) {
        return tap(in.uv);
    }
    let zoom = 1.0 / max(param(4u).x, 1.0);
    let at = 0.5 + (layer_uv(in.uv) - 0.5) * zoom;
    let drift = aim * px(param(1u).x) * mask_at(in.uv);
    let shift = (layer_uv(drift) - layer_uv(vec2<f32>(0.0))) * zoom;
    return tap(from_layer(relief(aux0, samp, at, shift, param(2u).x, param(3u).x)));
}
