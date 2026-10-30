fn shaft_place(p: vec2<f32>) -> vec3<f32> {
    let mode = u32(param(0u).x + 0.5);
    let a = radians(param(1u).x);
    let dir = vec2<f32>(cos(a), sin(a));
    if mode == 0u {
        let along = dot(p - 0.5, dir) / (abs(dir.x) + abs(dir.y)) + 0.5;
        return vec3<f32>(dot(p - 0.5, vec2<f32>(-dir.y, dir.x)) * 9.0, 0.0, along);
    }
    let corner = mode == 2u;
    let origin = select(param(2u).xy, round(clamp(param(2u).xy, vec2<f32>(0.0), vec2<f32>(1.0))), corner);
    let d = p - origin;
    let turn = atan2(d.y, d.x);
    return vec3<f32>(vec2<f32>(cos(turn), sin(turn)) * 4.0, length(d) / select(1.0, 1.4142, corner));
}

@fragment
fn fs_light_shafts(in: VertexOut) -> @location(0) vec4<f32> {
    let c = tap(in.uv);
    let place = shaft_place(layer_uv(in.uv));
    let t = frame.time * param(9u).x;
    let n = 0.65 * value_noise(place.xy + vec2<f32>(0.0, t)) + 0.35 * value_noise(place.xy * 2.3 + vec2<f32>(t * 1.7, 5.0));
    let shaped = pow(clamp(n, 0.0, 1.0), param(5u).x);
    let beam = mix(smoothstep(0.42, 0.58, shaped), shaped, param(8u).x);
    let f = max(param(7u).x, 1e-3);
    let fade = smoothstep(0.0, f * 0.25, place.z) * (1.0 - smoothstep(1.0 - f, 1.0, place.z));
    var tone = from_linear(param(3u).rgb) * param(3u).a;
    if textureDimensions(aux0).x > 1u {
        tone = textureSampleLevel(aux0, samp, vec2<f32>(clamp(place.z, 0.0, 1.0), 0.5), 0.0).rgb;
    }
    return vec4<f32>(c.rgb + tone * beam * fade * param(6u).x * mask_at(in.uv), c.a);
}
