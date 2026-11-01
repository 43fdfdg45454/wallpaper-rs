fn uv_transform(uv: vec2<f32>) -> vec2<f32> {
    let span = layer_extent();
    let p = (layer_uv(uv) - param(4u).xy - param(0u).xy) * span;
    let a = radians(param(2u).x);
    let turned = mat2x2<f32>(cos(a), -sin(a), sin(a), cos(a)) * p;
    let t = tan(radians(param(3u).xy));
    let straightened = mat2x2<f32>(1.0, -t.y, -t.x, 1.0) * turned / (1.0 - t.x * t.y);
    let q = from_layer(straightened / max(param(1u).xy, vec2<f32>(0.01)) / span + param(4u).xy);
    return select(OUTSIDE_UV, q, all(q >= vec2<f32>(0.0)) && all(q <= vec2<f32>(1.0)));
}
