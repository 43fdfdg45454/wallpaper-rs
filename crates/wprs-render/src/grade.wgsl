struct Grade { m0: vec4<f32>, m1: vec4<f32>, m2: vec4<f32>, lift: vec4<f32>, gain: vec4<f32> };

fn graded(c: vec3<f32>, g: Grade) -> vec3<f32> {
    if g.gain.w == 0.0 { return c; }
    let mixed = vec3(dot(g.m0.xyz, c) + g.m0.w, dot(g.m1.xyz, c) + g.m1.w, dot(g.m2.xyz, c) + g.m2.w);
    let p = (pow(max(mixed, vec3(0.0)), vec3(1.0 / 2.2)) - 0.5) * g.lift.w + 0.5;
    return pow(clamp(g.lift.rgb + p * (g.gain.rgb - g.lift.rgb), vec3(0.0), vec3(1.0)), vec3(2.2));
}
