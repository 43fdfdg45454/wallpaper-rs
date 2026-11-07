fn tap(uv: vec2<f32>) -> vec4<f32> {
    if fx.view[0].w < 0.5 {
        return sample(input_tex, uv);
    }
    let p = vec3<f32>(clamp(uv, fx.clip.xy, fx.clip.zw), 1.0);
    let lu = vec2<f32>(dot(fx.view[0].xyz, p), dot(fx.view[1].xyz, p));
    let r = fx.view[2];
    let room = 0.5 / max(vec2<f32>(textureDimensions(input_tex)) * r.zw, vec2<f32>(1.0));
    let t = textureSampleLevel(input_tex, samp, r.xy + clamp(lu, room, 1.0 - room) * r.zw, 0.0);
    let inside = all(lu >= vec2<f32>(0.0)) && all(lu <= vec2<f32>(1.0));
    return select(vec4<f32>(0.0), select(t, vec4<f32>(t.rgb * t.a, t.a), fx.view[0].w < 1.5), inside);
}
