struct Light { at: vec4<f32>, color: vec4<f32>, dir: vec4<f32>, more: vec4<f32> };
struct Scene3 {
    clip: mat4x4<f32>,
    eye: vec4<f32>,
    forward: vec4<f32>,
    ambient: vec4<f32>,
    sky: vec4<f32>,
    up: vec4<f32>,
    slice: vec4<f32>,
    lights: array<Light, 12>,
};
struct Object { model: mat4x4<f32>, normal: mat4x4<f32> };
struct Finish { color: vec4<f32>, emissive: vec4<f32>, pbr: vec4<f32>, flags: vec4<f32>, maps: vec4<f32>, more: vec4<f32> };
struct In {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec4<f32>,
};
struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec4<f32>,
};

@group(0) @binding(0) var<uniform> scene: Scene3;
@group(0) @binding(1) var env: texture_2d<f32>;
@group(0) @binding(2) var env_smp: sampler;
@group(1) @binding(0) var<uniform> object: Object;
@group(2) @binding(0) var base_map: texture_2d<f32>;
@group(2) @binding(1) var orm_map: texture_2d<f32>;
@group(2) @binding(2) var normal_map: texture_2d<f32>;
@group(2) @binding(3) var occlusion_map: texture_2d<f32>;
@group(2) @binding(4) var emissive_map: texture_2d<f32>;
@group(2) @binding(5) var smp: sampler;
@group(3) @binding(0) var<uniform> finish: Finish;

const PI: f32 = 3.14159265;

@vertex
fn vs_solid(v: In) -> Out {
    let world = object.model * vec4<f32>(v.pos, 1.0);
    var o: Out;
    o.position = scene.clip * world;
    o.world = world.xyz;
    o.normal = normalize((object.normal * vec4<f32>(v.normal, 0.0)).xyz);
    o.tangent = vec4<f32>(normalize((object.model * vec4<f32>(v.tangent.xyz, 0.0)).xyz), v.tangent.w * object.normal[3][3]);
    o.uv = v.uv;
    return o;
}

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

@fragment
fn fs_solid(in: Out, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let along = dot(in.world - scene.eye.xyz, scene.forward.xyz);
    if along < scene.slice.x || along >= scene.slice.y {
        discard;
    }
    var albedo = finish.color * textureSample(base_map, smp, in.uv);
    if finish.flags.x > 0.0 && albedo.a < finish.flags.x {
        discard;
    }
    let orm = textureSample(orm_map, smp, in.uv);
    let ao = mix(1.0, textureSample(occlusion_map, smp, in.uv).r, finish.pbr.w);
    let metal = clamp(finish.pbr.x * orm.b, 0.0, 1.0);
    let rough = clamp(finish.pbr.y * orm.g, 0.04, 1.0);
    let glow = finish.emissive.rgb * textureSample(emissive_map, smp, in.uv).rgb;
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
    if finish.flags.y < 0.5 {
        let flat = albedo.rgb + glow;
        return vec4<f32>(flat * albedo.a, albedo.a) * finish.more.x;
    }
    let v = normalize(scene.eye.xyz - in.world);
    let n_v = max(dot(n, v), 1e-4);
    let f0 = mix(vec3<f32>(0.04), albedo.rgb, metal);
    let diffuse = albedo.rgb * (1.0 - metal);
    let a = rough * rough;
    var light = vec3<f32>(0.0);
    for (var i = 0u; i < u32(scene.eye.w); i++) {
        let l = scene.lights[i];
        let f = falloff(l, in.world);
        let n_l = max(dot(n, f.xyz), 0.0);
        if n_l <= 0.0 || f.w <= 0.0 {
            continue;
        }
        let h = normalize(f.xyz + v);
        let fresnel = f0 + (1.0 - f0) * pow(1.0 - max(dot(h, v), 0.0), 5.0);
        let spec = fresnel * ggx(max(dot(n, h), 0.0), a) * smith(n_v, n_l, a) / max(4.0 * n_v * n_l, 1e-4);
        light += (diffuse / PI * (1.0 - fresnel) + spec) * l.color.rgb * n_l * f.w * PI;
    }
    let up = dot(n, scene.up.xyz) * 0.5 + 0.5;
    var around = (scene.ambient.rgb + scene.sky.rgb * up) * diffuse * ao;
    if scene.slice.z > 0.5 {
        let levels = f32(textureNumLevels(env));
        let seen = textureSampleLevel(env, env_smp, equirect(n), levels - 1.0).rgb;
        around += seen * diffuse * ao * scene.ambient.w;
        if scene.slice.w > 0.5 {
            let r = reflect(-v, n);
            let shine = textureSampleLevel(env, env_smp, equirect(r), rough * (levels - 1.0)).rgb;
            let fresnel = f0 + (max(vec3<f32>(1.0 - rough), f0) - f0) * pow(1.0 - n_v, 5.0);
            around += shine * fresnel * ao * scene.ambient.w;
        }
    }
    let color = light + around + glow;
    return vec4<f32>(color * albedo.a, albedo.a) * finish.more.x;
}
