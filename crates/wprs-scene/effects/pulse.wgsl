fn color_pulse(color: vec4<f32>, uv: vec2<f32>) -> vec4<f32> {
    let shift = time_phase(aux0, uv);
    var v = 0.5 + 0.5 * cos(TAU * (frame.time * param(1u).x + shift));
    let heard = u32(param(8u).x + 0.5);
    if heard > 0u {
        v = clamp(band(heard - 1u), 0.0, 1.0);
    }
    let wobble = noise1(frame.time * param(6u).x + shift * 17.0 + frame.seed);
    v = clamp(mix(v, wobble, param(5u).x), 0.0, 1.0);
    let m = mask_at(uv);
    let k = mix(1.0, mix(param(2u).x, param(2u).y, v), m);
    switch u32(param(0u).x + 0.5) {
        case 1u: {
            let tint = mix(param(3u).rgb, param(4u).rgb, v);
            return vec4<f32>(color.rgb * mix(vec3<f32>(1.0), from_linear(tint), m), color.a);
        }
        case 2u: { return color * min(k, 1.0); }
        default: { return vec4<f32>(min(color.rgb * k, vec3<f32>(color.a)), color.a); }
    }
}
