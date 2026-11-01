@fragment
fn fs_reveal(in: VertexOut) -> @location(0) vec4<f32> {
    let at = layer_uv(mix(from_layer(param(6u).xy), cursor_uv(), param(4u).x * frame.presence));
    let span = layer_extent();
    let round = span / max(min(span.x, span.y), 1.0);
    let d = (layer_uv(in.uv) - at) * select(round, vec2<f32>(1.0), param(1u).x > 0.5);
    let r = param(2u).x;
    let w = 1.0 - smoothstep(r * (1.0 - param(3u).x) - 1e-4, r, length(d));
    let shown = select(w, 1.0 - w, param(5u).x > 0.5) * mask_at(in.uv);
    return mix(tap(in.uv), asset_at(aux0, in.uv), shown);
}
