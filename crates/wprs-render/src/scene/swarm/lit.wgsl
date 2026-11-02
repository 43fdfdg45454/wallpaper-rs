@group(2) @binding(0) var<uniform> lights: Lights;
@group(2) @binding(1) var normal_map: texture_2d<f32>;
@group(2) @binding(2) var bend_map: texture_2d<f32>;

fn turned(t: vec2<f32>, turn: vec4<f32>) -> vec2<f32> {
    return t.x * turn.xy - t.y * vec2<f32>(-turn.y, turn.x);
}

@fragment
fn fs_lit_particle(in: Corner) -> @location(0) vec4<f32> {
    let c = painted(in);
    if c.a <= 1e-5 {
        return c;
    }
    let albedo = c.rgb / c.a;
    var n = vec3<f32>(0.0, 0.0, 1.0);
    if look.light.y > 0.5 {
        let t = textureSampleLevel(normal_map, samp, in.uv, 0.0).xyz * 2.0 - 1.0;
        n = normalize(vec3<f32>(turned(t.xy, in.turn), max(t.z, 0.0) + 1e-3));
    }
    let p = in.position.xy + look.place.xy;
    var light = mix(lights.ambient.rgb, lights.sky.rgb, 0.5 - 0.5 * n.y);
    for (var i = 0u; i < min(u32(lights.head.x), 8u); i++) {
        let b = lights.bulbs[i];
        let r = reach(b, p);
        if r.w <= 0.0 {
            continue;
        }
        let facing = vec3<f32>(n.xy, select(n.z, -n.z, r.z < 0.0));
        light += b.color.rgb * max(dot(facing, r.xyz), 0.0) * r.w;
    }
    let rgb = albedo * light + albedo * look.place.w;
    return vec4<f32>(mix(albedo, rgb, look.place.z) * c.a, c.a);
}

@fragment
fn fs_bend_particle(in: Corner) -> @location(0) vec4<f32> {
    let a = painted(in).a;
    let t = textureSampleLevel(bend_map, samp, in.uv, 0.0).xy * 2.0 - 1.0;
    let shift = turned(t, in.turn) * look.bend.x * 0.5 * in.turn.z;
    return vec4<f32>(shift * a, 0.0, a);
}
