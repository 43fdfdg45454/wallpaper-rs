struct Bulb {
    at: vec4<f32>,
    color: vec4<f32>,
    dir: vec4<f32>,
    more: vec4<f32>,
};

struct Lights {
    head: vec4<f32>,
    ambient: vec4<f32>,
    sky: vec4<f32>,
    bulbs: array<Bulb, 12>,
};

@group(2) @binding(0) var<uniform> lights: Lights;
@group(2) @binding(1) var normal_tex: texture_2d<f32>;
@group(2) @binding(2) var rough_tex: texture_2d<f32>;
@group(2) @binding(3) var glow_tex: texture_2d<f32>;
@group(2) @binding(4) var env_tex: texture_2d<f32>;

fn facing(at: vec2<f32>) -> vec3<f32> {
    if item.lit_maps.x < 0.5 {
        return vec3<f32>(0.0, 0.0, 1.0);
    }
    let t = textureSampleLevel(normal_tex, samp, at, 0.0).xyz * 2.0 - 1.0;
    let xy = t.x * item.lit_basis.xy - t.y * item.lit_basis.zw;
    return normalize(vec3<f32>(xy * item.lit_at.z, max(t.z, 0.0) + 1e-3));
}

fn reach(b: Bulb, p: vec2<f32>) -> vec4<f32> {
    let kind = b.color.w;
    if kind > 2.5 {
        return vec4<f32>(-b.dir.xyz, 1.0);
    }
    var src = b.at.xy;
    if kind > 1.5 {
        let ab = b.more.yz * 2.0;
        let t = clamp(dot(p - (src - b.more.yz), ab) / max(dot(ab, ab), 1e-6), 0.0, 1.0);
        src = src - b.more.yz + ab * t;
    }
    let v = vec3<f32>(src - p, b.at.z);
    let d = length(v);
    if d >= b.at.w {
        return vec4<f32>(0.0);
    }
    let f = 1.0 - d * d / (b.at.w * b.at.w);
    let l = v / max(d, 1e-3);
    var k = f * f;
    if kind > 0.5 && kind < 1.5 {
        k *= smoothstep(b.dir.w, b.more.x, dot(-l, b.dir.xyz));
    }
    return vec4<f32>(l, k);
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
        light += b.color.rgb * ndl * r.w;
        let h = normalize(r.xyz + vec3<f32>(0.0, 0.0, 1.0));
        spec += b.color.rgb * pow(max(dot(n, h), 0.0), shine) * gloss * r.w * select(0.0, 1.0, ndl > 0.0);
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
