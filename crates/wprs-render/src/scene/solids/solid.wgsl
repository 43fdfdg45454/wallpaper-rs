fn ggx(n_h: f32, a: f32) -> f32 {
    let a2 = a * a;
    let d = n_h * n_h * (a2 - 1.0) + 1.0;
    return a2 / max(PI * d * d, 1e-6);
}

fn smith(n_v: f32, n_l: f32, a: f32) -> f32 {
    let k = (a + 1.0) * (a + 1.0) / 8.0;
    return n_v / (n_v * (1.0 - k) + k) * n_l / (n_l * (1.0 - k) + k);
}

fn equirect(d: vec3<f32>) -> vec2<f32> {
    return vec2<f32>(atan2(d.x, -d.z) / (2.0 * PI) + 0.5, acos(clamp(d.y, -1.0, 1.0)) / PI);
}

fn falloff(l: Light, p: vec3<f32>) -> vec4<f32> {
    let kind = l.color.w;
    if kind > 2.5 {
        return vec4<f32>(-l.dir.xyz, 1.0);
    }
    var at = l.at.xyz;
    if kind > 1.5 {
        let half = l.more.y * 0.5;
        at = at + clamp(dot(p - at, l.dir.xyz), -half, half) * l.dir.xyz;
    }
    let to = at - p;
    let d = length(to);
    let k = clamp(1.0 - pow(d / max(l.at.w, 1e-3), 2.0), 0.0, 1.0);
    var power = k * k;
    let dir = to / max(d, 1e-6);
    if kind > 0.5 && kind < 1.5 {
        power = power * smoothstep(l.dir.w, l.more.x, dot(-dir, l.dir.xyz));
    }
    return vec4<f32>(dir, power);
}

fn tap(s: u32, uv: vec2<f32>, layer: u32, depth: f32) -> f32 {
    if s == 0u {
        return textureSampleCompareLevel(shadow_a, shadow_cmp, uv, layer, depth);
    }
    return textureSampleCompareLevel(shadow_b, shadow_cmp, uv, layer, depth);
}

fn shadowed(l: Light, p: vec3<f32>, n: vec3<f32>) -> f32 {
    let s = i32(l.more.z) - 1;
    if s < 0 || finish.flags.z < 0.5 {
        return 1.0;
    }
    let info = shade.info[s];
    var face = 0u;
    if info.x > 1.5 {
        let d = p - shade.at[s].xyz;
        let a = abs(d);
        if a.x >= a.y && a.x >= a.z {
            face = select(1u, 0u, d.x > 0.0);
        } else if a.y >= a.z {
            face = select(3u, 2u, d.y > 0.0);
        } else {
            face = select(5u, 4u, d.z > 0.0);
        }
    }
    let lift = n * info.w * shade.at[s].w;
    let q = shade.mats[u32(info.y) + face] * vec4<f32>(p + lift, 1.0);
    let c = q.xyz / q.w;
    let uv = c.xy * vec2<f32>(0.5, -0.5) + 0.5;
    if q.w <= 0.0 || any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) || c.z > 1.0 {
        return 1.0;
    }
    let step = info.z * 0.5;
    var sum = 0.0;
    for (var k = 0u; k < 4u; k++) {
        let o = vec2<f32>(select(-step, step, (k & 1u) == 1u), select(-step, step, k > 1u));
        sum += tap(u32(s), uv + o, face, c.z - info.w * 0.002);
    }
    return sum * 0.25;
}

fn hashed(c: vec2<f32>) -> f32 {
    return fract(sin(dot(c, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

@fragment
fn fs_solid(in: Out, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let along = dot(in.world - scene.eye.xyz, scene.forward.xyz);
    if along < scene.slice.x || along >= scene.slice.y {
        discard;
    }
    let kind = finish.shade.x;
    var s: Surface;
    s.albedo = finish.color * textureSample(base_map, smp, in.uv);
    let orm = textureSample(orm_map, smp, in.uv);
    s.occlusion = mix(1.0, textureSample(occlusion_map, smp, in.uv).r, finish.pbr.w);
    s.metallic = clamp(finish.pbr.x * orm.b, 0.0, 1.0);
    s.roughness = clamp(finish.pbr.y * orm.g, 0.04, 1.0);
    s.emissive = finish.emissive.rgb * textureSample(emissive_map, smp, in.uv).rgb;
    s.position = in.world;
    s.uv = in.uv;
    var n = normalize(in.normal);
    if !front {
        n = -n;
    }
    if finish.maps.x > 0.5 {
        let t = normalize(in.tangent.xyz - n * dot(n, in.tangent.xyz));
        let b = cross(n, t) * select(1.0, -1.0, in.tangent.w < 0.0);
        let m = textureSample(normal_map, smp, in.uv).xyz * 2.0 - 1.0;
        n = normalize(t * m.x * finish.pbr.z + b * m.y * finish.pbr.z + n * m.z);
    }
    s.normal = n;
    if kind > 1.5 && kind < 2.5 && in.shell > 0.5 {
        let k = in.shell / (max(finish.shade.y, 1.0) + 1.0);
        let cells = floor(in.uv * (40.0 + finish.shade.w * 360.0));
        if hashed(cells) < k + (1.0 - finish.shade.w) * 0.5 {
            discard;
        }
        s.occlusion = s.occlusion * mix(0.4, 1.0, k);
    }
    if kind > 3.5 && kind < 4.5 {
        let away = distance(s.albedo.rgb, finish.key.rgb);
        s.albedo.a = s.albedo.a * smoothstep(finish.key.w, finish.key.w + max(finish.more.y, 1e-4), away);
    }
    s = material(s);
    if finish.flags.x > 0.0 && s.albedo.a < finish.flags.x {
        discard;
    }
    if finish.flags.w > 0.5 {
        s.albedo.a = finish.flags.w - 1.0;
    } else if s.albedo.a < 0.004 {
        discard;
    }
    return lit(s, in, front);
}
