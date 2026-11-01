fn color_shine(color: vec4<f32>, uv: vec2<f32>) -> vec4<f32> {
    let a = radians(param(0u).x);
    let dir = vec2<f32>(cos(a), sin(a));
    let along = dot(layer_uv(uv) - 0.5, dir) / (abs(dir.x) + abs(dir.y)) + 0.5;
    let cycle = 1.0 + param(6u).x;
    let at = frame.time * param(5u).x + param(7u).x;
    let w = param(1u).x;
    let center = mix(-w, 1.0 + w, at - cycle * floor(at / cycle));
    let band = 1.0 - smoothstep(w * 0.5 * (1.0 - param(2u).x), w * 0.5 + 1e-4, abs(along - center));
    var tone = from_linear(param(4u).rgb) * param(4u).a;
    if textureDimensions(aux0).x > 1u {
        let across = clamp((along - center) / max(w, 1e-4) + 0.5, 0.0, 1.0);
        tone = textureSampleLevel(aux0, samp, vec2<f32>(across, 0.5), 0.0).rgb;
    }
    let strength = band * param(3u).x * mask_at(uv);
    let mode = u32(param(8u).x + 0.5);
    if mode == 1u {
        return vec4<f32>(color.rgb + tone * strength * color.a, color.a);
    }
    let c = straight(color);
    let rgb = clamp(blend_mode(mode, c.rgb, tone, min(strength, 1.0)), vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(rgb * c.a, c.a);
}
