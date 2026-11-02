@group(2) @binding(0) var<uniform> lights: Lights;
@group(2) @binding(1) var normal_tex: texture_2d<f32>;
@group(2) @binding(2) var rough_tex: texture_2d<f32>;
@group(2) @binding(3) var glow_tex: texture_2d<f32>;
@group(2) @binding(4) var env_tex: texture_2d<f32>;
@group(2) @binding(5) var gobo_a: texture_2d<f32>;
@group(2) @binding(6) var gobo_b: texture_2d<f32>;
@group(2) @binding(7) var gobo_c: texture_2d<f32>;
@group(2) @binding(8) var gobo_d: texture_2d<f32>;

fn gobo(b: Bulb, l: vec3<f32>) -> vec3<f32> {
    let slot = u32(b.air.z + 0.5);
    if slot == 0u || b.color.w < 0.5 || b.color.w > 1.5 {
        return vec3<f32>(1.0);
    }
    let a = b.dir.xyz;
    let level = length(a.xy) > 1e-4;
    let side = select(vec3<f32>(1.0, 0.0, 0.0), normalize(vec3<f32>(a.y, -a.x, 0.0)), level);
    let up = cross(side, a);
    let w = -l;
    let wide = sqrt(max(1.0 - b.dir.w * b.dir.w, 1e-6)) / max(b.dir.w, 1e-3);
    let uv = 0.5 + 0.5 * vec2<f32>(dot(w, side), dot(w, up)) / (max(dot(w, a), 1e-3) * wide);
    var c = textureSampleLevel(gobo_a, samp, uv, 0.0);
    if slot == 2u {
        c = textureSampleLevel(gobo_b, samp, uv, 0.0);
    } else if slot == 3u {
        c = textureSampleLevel(gobo_c, samp, uv, 0.0);
    } else if slot == 4u {
        c = textureSampleLevel(gobo_d, samp, uv, 0.0);
    }
    return c.rgb;
}

fn facing(at: vec2<f32>) -> vec3<f32> {
    if item.lit_maps.x < 0.5 {
        return vec3<f32>(0.0, 0.0, 1.0);
    }
    let t = textureSampleLevel(normal_tex, samp, at, 0.0).xyz * 2.0 - 1.0;
    let xy = t.x * item.lit_basis.xy - t.y * item.lit_basis.zw;
    return normalize(vec3<f32>(xy * item.lit_at.z, max(t.z, 0.0) + 1e-3));
}

fn shade(c: vec4<f32>, at: vec2<f32>, in: Out) -> vec4<f32> {
    if c.a <= 1e-5 {
        return c;
    }
    let albedo = c.rgb / c.a;
    let n = facing(at);
    let rough = item.lit.y * select(1.0, textureSampleLevel(rough_tex, samp, at, 0.0).r, item.lit_maps.y > 0.5);
    let glow = item.lit.w * select(1.0, textureSampleLevel(glow_tex, samp, at, 0.0).r, item.lit_maps.z > 0.5);
    let p = in.position.xy + item.lit_at.xy;
    var light = mix(lights.ambient.rgb, lights.sky.rgb, 0.5 - 0.5 * n.y);
    var spec = vec3<f32>(0.0);
    let shine = mix(256.0, 4.0, rough);
    let gloss = (1.0 - rough) * (1.0 - rough);
    for (var i = 0u; i < u32(lights.head.x); i++) {
        let b = lights.bulbs[i];
        let r = reach(b, p);
        if r.w <= 0.0 {
            continue;
        }
        let ndl = max(dot(n, r.xyz), 0.0);
        let tone = b.color.rgb * gobo(b, r.xyz);
        light += tone * ndl * r.w;
        let h = normalize(r.xyz + vec3<f32>(0.0, 0.0, 1.0));
        spec += tone * pow(max(dot(n, h), 0.0), shine) * gloss * r.w * select(0.0, 1.0, ndl > 0.0);
    }
    var rgb = albedo * light + spec + albedo * glow;
    if item.lit_maps.w > 0.5 {
        var uv = vec2<f32>(0.5 + 0.5 * n.x, 0.5 + 0.5 * n.y);
        if item.lit_maps.w > 1.5 {
            uv = item.lit_env.xy + (in.position.xy + n.xy * item.lit_at.w) * item.lit_env.zw;
        }
        let e = textureSampleLevel(env_tex, samp, uv, 0.0);
        rgb = mix(rgb, select(e.rgb, e.rgb / e.a, e.a > 1e-5), item.lit.z * (1.0 - 0.5 * rough));
    }
    return vec4<f32>(mix(albedo, rgb, item.lit.x) * c.a, c.a);
}

@fragment
fn fs_lit_picture(in: Out) -> @location(0) vec4<f32> {
    let at = picture_at(in);
    return finish(shade(picture(in, at), at, in), in);
}

@fragment
fn fs_lit_texture(in: Out) -> @location(0) vec4<f32> {
    let c = styled(textureSample(src_tex, samp, item.uv.xy + in.uv * item.uv.zw)) * item.color;
    return finish(shade(c, in.uv, in), in);
}

@fragment
fn fs_lit_text(in: Out) -> @location(0) vec4<f32> {
    return finish(shade(lettered(in), in.uv, in), in);
}
