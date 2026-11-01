fn lens_flare_disc(d: f32, r: f32) -> f32 {
    return smoothstep(r, r * 0.6, d) * 0.6 + exp(-d * d / (r * r * 0.15)) * 0.4;
}

@fragment
fn fs_lens_flare(in: VertexOut) -> @location(0) vec4<f32> {
    let c = tap(in.uv);
    let span = layer_extent();
    let round = span / max(max(span.x, span.y), 1.0);
    let s = layer_uv(mix(from_layer(param(0u).xy), cursor_uv(), param(1u).x * frame.presence));
    let p = layer_uv(in.uv);
    let axis = vec2<f32>(0.5) - s;
    var light = vec3<f32>(0.0);
    let n = clamp(i32(param(3u).x + 0.5), 0, 8);
    for (var i = 1; i <= min(n, 8); i++) {
        let k = f32(i);
        let at = s + axis * (0.4 + k * 0.45) * (1.0 + 0.3 * hash(vec2<f32>(k, 3.1)));
        let r = 0.02 + 0.07 * hash(vec2<f32>(k, 7.7));
        let hue = vec3<f32>(0.6 + 0.4 * sin(k * 1.9), 0.6 + 0.4 * sin(k * 1.9 + 2.1), 0.6 + 0.4 * sin(k * 1.9 + 4.2));
        light += hue * lens_flare_disc(length((p - at) * round), r) * 0.35;
    }
    let ring = length((p - s) * round);
    light += vec3<f32>(exp(-pow((ring - 0.25) / 0.02, 2.0)) * param(4u).x * 0.5);
    light += vec3<f32>(exp(-ring * ring * 300.0));
    let tone = from_linear(param(5u).rgb) * param(5u).a * param(2u).x;
    let add = light * tone;
    return vec4<f32>(c.rgb + add, min(max(c.a, max(add.r, max(add.g, add.b))), 1.0));
}
