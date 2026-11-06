fn shine_band(d: f32, w: f32) -> f32 {
    return 1.0 - smoothstep(w * 0.5 * (1.0 - param(2u).x), w * 0.5 + 1e-4, d);
}

fn shine_glint(p: vec2<f32>, at: vec2<f32>, dir: vec2<f32>, w: f32) -> vec2<f32> {
    let span = layer_extent();
    let aspect = vec2<f32>(span.x / max(span.y, 1.0), 1.0);
    let q = (p - at) * aspect;
    let side = vec2<f32>(-dir.y, dir.x);
    let a = abs(dot(q, dir));
    let b = abs(dot(q, side));
    let core = shine_band(length(q), w * 0.6);
    let reach = w * 1.5;
    let rays = max(max(1.0 - a / reach, 0.0) * exp(-b * 120.0), max(1.0 - b / reach, 0.0) * exp(-a * 120.0));
    return vec2<f32>(max(core, rays), clamp(dot(q, dir) / max(w, 1e-4) + 0.5, 0.0, 1.0));
}

fn color_shine(color: vec4<f32>, uv: vec2<f32>) -> vec4<f32> {
    let a = radians(param(0u).x);
    let dir = vec2<f32>(cos(a), sin(a));
    let p = layer_uv(uv);
    let style = u32(param(10u).x + 0.5);
    var along = dot(p - 0.5, dir) / (abs(dir.x) + abs(dir.y)) + 0.5;
    if style == 2u {
        along = length(p - 0.5) * 1.41421356;
    }
    let cycle = 1.0 + param(6u).x;
    let at = frame.time * param(5u).x + param(7u).x;
    let w = param(1u).x;
    let center = mix(-w, 1.0 + w, at - cycle * floor(at / cycle));
    var band = shine_band(abs(along - center), w);
    var across = clamp((along - center) / max(w, 1e-4) + 0.5, 0.0, 1.0);
    if style == 1u {
        band = max(band, 0.6 * shine_band(abs(along - center + w * 1.6), w * 0.5));
    }
    if style == 3u {
        let spot = 0.5 + dir * (center - 0.5) * (abs(dir.x) + abs(dir.y));
        let glint = shine_glint(p, spot, dir, w);
        band = glint.x;
        across = glint.y;
    }
    var tone = from_linear(param(4u).rgb) * param(4u).a;
    if textureDimensions(aux0).x > 1u {
        tone = textureSampleLevel(aux0, samp, vec2<f32>(across, 0.5), 0.0).rgb;
    }
    let strength = band * param(3u).x * mask_at(uv);
    let mode = u32(param(8u).x + 0.5);
    if mode == 1u {
        return vec4<f32>(color.rgb + tone * strength * color.a, color.a);
    }
    let c = straight(color);
    let rgb = max(blend_mode(mode, c.rgb, tone, min(strength, 1.0)), vec3<f32>(0.0));
    return vec4<f32>(rgb * c.a, c.a);
}
