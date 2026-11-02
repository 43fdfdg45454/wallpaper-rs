fn shown(c: vec3<f32>, a: f32) -> vec4<f32> {
    return vec4<f32>(c * (1.0 - scene.fade.x) * a, a) * finish.more.x;
}

fn lit(s: Surface, in: Out, front: bool) -> vec4<f32> {
    let albedo = s.albedo;
    let n = normalize(s.normal);
    let debug = scene.fade.y;
    if debug > 0.5 && debug < 1.5 {
        return vec4<f32>(vec3<f32>(clamp(in.paint, 0.0, 1.0)), 1.0) * finish.more.x;
    }
    if debug > 1.5 && debug < 2.5 {
        return vec4<f32>(vec3<f32>(albedo.a), 1.0) * finish.more.x;
    }
    if finish.flags.y < 0.5 {
        return shown(albedo.rgb + s.emissive, albedo.a);
    }
    let v = normalize(scene.eye.xyz - s.position);
    let n_v = max(dot(n, v), 1e-4);
    let f0 = mix(vec3<f32>(0.04), albedo.rgb, s.metallic);
    let diffuse = albedo.rgb * (1.0 - s.metallic);
    let a = s.roughness * s.roughness;
    var light = vec3<f32>(0.0);
    var shadow = 1.0;
    for (var i = 0u; i < u32(scene.eye.w); i++) {
        let l = scene.lights[i];
        let f = falloff(l, s.position);
        let n_l = max(dot(n, f.xyz), 0.0);
        if n_l <= 0.0 || f.w <= 0.0 {
            continue;
        }
        let dark = shadowed(l, s.position, n);
        shadow = min(shadow, dark);
        let h = normalize(f.xyz + v);
        let fresnel = f0 + (1.0 - f0) * pow(1.0 - max(dot(h, v), 0.0), 5.0);
        let spec = fresnel * ggx(max(dot(n, h), 0.0), a) * smith(n_v, n_l, a) / max(4.0 * n_v * n_l, 1e-4);
        light += (diffuse / PI * (1.0 - fresnel) + spec) * l.color.rgb * n_l * f.w * PI * dark;
    }
    if debug > 2.5 {
        return vec4<f32>(mix(vec3<f32>(0.9, 0.2, 0.4), vec3<f32>(1.0), shadow), 1.0) * finish.more.x;
    }
    let ao = s.occlusion;
    let up = dot(n, scene.up.xyz) * 0.5 + 0.5;
    var around = (scene.ambient.rgb + scene.sky.rgb * up) * diffuse * ao;
    if scene.slice.z > 0.5 {
        let levels = f32(textureNumLevels(env));
        let seen = textureSampleLevel(env, env_smp, equirect(n), levels - 1.0).rgb;
        around += seen * diffuse * ao * scene.ambient.w;
        if scene.slice.w > 0.5 {
            let r = reflect(-v, n);
            let gleam = textureSampleLevel(env, env_smp, equirect(r), s.roughness * (levels - 1.0)).rgb;
            let fresnel = f0 + (max(vec3<f32>(1.0 - s.roughness), f0) - f0) * pow(1.0 - n_v, 5.0);
            around += gleam * fresnel * ao * scene.ambient.w;
        }
    }
    return shown(light + around + s.emissive, albedo.a);
}
