struct Light { at: vec4<f32>, color: vec4<f32>, dir: vec4<f32>, more: vec4<f32> };
struct Scene3 {
    clip: mat4x4<f32>,
    eye: vec4<f32>,
    forward: vec4<f32>,
    ambient: vec4<f32>,
    sky: vec4<f32>,
    up: vec4<f32>,
    slice: vec4<f32>,
    wind: vec4<f32>,
    fade: vec4<f32>,
    lights: array<Light, 12>,
};
struct Object { model: mat4x4<f32>, normal: mat4x4<f32> };
struct Bones { m: array<vec4<f32>, 768> };
struct Draw { first: vec4<f32>, slots: array<vec4<f32>, 4> };
struct Shadows { mats: array<mat4x4<f32>, 12>, info: array<vec4<f32>, 2>, at: array<vec4<f32>, 2> };
struct Finish {
    color: vec4<f32>,
    emissive: vec4<f32>,
    pbr: vec4<f32>,
    flags: vec4<f32>,
    maps: vec4<f32>,
    more: vec4<f32>,
    shade: vec4<f32>,
    comb: vec4<f32>,
    key: vec4<f32>,
    params: array<vec4<f32>, 8>,
};
struct In {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec4<f32>,
    @location(4) paint: f32,
};
struct Bound { @location(5) joints: vec4<f32>, @location(6) weights: vec4<f32> };
struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec4<f32>,
    @location(4) paint: f32,
    @location(5) @interpolate(flat) shell: f32,
};
struct Placed { p: vec3<f32>, n: vec3<f32>, t: vec3<f32> };

@group(0) @binding(0) var<uniform> scene: Scene3;
@group(0) @binding(1) var env: texture_2d<f32>;
@group(0) @binding(2) var env_smp: sampler;
@group(0) @binding(4) var shadow_a: texture_depth_2d_array;
@group(0) @binding(5) var shadow_b: texture_depth_2d_array;
@group(0) @binding(6) var shadow_cmp: sampler_comparison;
@group(0) @binding(7) var<uniform> shade: Shadows;
@group(1) @binding(0) var<uniform> object: Object;
@group(1) @binding(1) var<uniform> bones: Bones;
@group(1) @binding(2) var<uniform> draw: Draw;
@group(1) @binding(3) var morphs: texture_2d<f32>;
@group(2) @binding(0) var base_map: texture_2d<f32>;
@group(2) @binding(1) var orm_map: texture_2d<f32>;
@group(2) @binding(2) var normal_map: texture_2d<f32>;
@group(2) @binding(3) var occlusion_map: texture_2d<f32>;
@group(2) @binding(4) var emissive_map: texture_2d<f32>;
@group(2) @binding(5) var smp: sampler;
@group(3) @binding(0) var<uniform> finish: Finish;

const PI: f32 = 3.14159265;

fn place(p: vec3<f32>, n: vec3<f32>, t: vec3<f32>, paint: f32, shell: u32) -> Placed {
    var o: Placed;
    o.p = (object.model * vec4<f32>(p, 1.0)).xyz;
    o.n = normalize((object.normal * vec4<f32>(n, 0.0)).xyz);
    o.t = normalize((object.model * vec4<f32>(t, 0.0)).xyz);
    let kind = finish.shade.x;
    if kind > 1.5 && kind < 2.5 && shell > 0u {
        let k = f32(shell) / max(finish.shade.y, 1.0);
        let comb = (object.model * vec4<f32>(finish.comb.xyz, 0.0)).xyz;
        o.p = o.p + (o.n * k + comb * k * k) * finish.shade.z;
    }
    if kind > 2.5 && kind < 3.5 {
        let phase = dot(o.p, vec3<f32>(0.7, 0.3, 0.5)) * 3.0;
        let wave = sin(scene.wind.w * finish.more.z * 2.0 + phase) * 0.5 + 0.6;
        o.p = o.p + scene.wind.xyz * finish.comb.w * clamp(paint, 0.0, 1.0) * wave;
    }
    return o;
}

fn skin(p: vec4<f32>, b: Bound) -> vec3<f32> {
    var o = vec3<f32>(0.0);
    for (var k = 0u; k < 4u; k++) {
        let i = u32(b.joints[k]) * 3u;
        o += vec3<f32>(dot(bones.m[i], p), dot(bones.m[i + 1u], p), dot(bones.m[i + 2u], p)) * b.weights[k];
    }
    return o;
}

fn delta(at: u32) -> vec3<f32> {
    return textureLoad(morphs, vec2<i32>(i32(at % 2048u), i32(at / 2048u)), 0).xyz;
}

fn out(o: Placed, v: In, shell: u32) -> Out {
    var r: Out;
    r.position = scene.clip * vec4<f32>(o.p, 1.0);
    r.world = o.p;
    r.normal = o.n;
    r.tangent = vec4<f32>(o.t, v.tangent.w * object.normal[3][3]);
    r.uv = v.uv;
    r.paint = v.paint;
    r.shell = f32(shell);
    return r;
}

fn posed(v: In, b: Bound, index: u32) -> array<vec3<f32>, 3> {
    var p = v.pos;
    var n = v.normal;
    let local = index - u32(draw.first.x);
    for (var s = 0u; s < 4u; s++) {
        let pair = draw.slots[s];
        if pair.y != 0.0 {
            p += delta(u32(pair.x) + local * 2u) * pair.y;
            n += delta(u32(pair.x) + local * 2u + 1u) * pair.y;
        }
        if pair.w != 0.0 {
            p += delta(u32(pair.z) + local * 2u) * pair.w;
            n += delta(u32(pair.z) + local * 2u + 1u) * pair.w;
        }
    }
    return array<vec3<f32>, 3>(skin(vec4<f32>(p, 1.0), b), skin(vec4<f32>(n, 0.0), b), skin(vec4<f32>(v.tangent.xyz, 0.0), b));
}

@vertex
fn vs_solid(v: In, @builtin(instance_index) shell: u32) -> Out {
    return out(place(v.pos, v.normal, v.tangent.xyz, v.paint, shell), v, shell);
}

@vertex
fn vs_skinned(v: In, b: Bound, @builtin(instance_index) shell: u32, @builtin(vertex_index) index: u32) -> Out {
    let q = posed(v, b, index);
    return out(place(q[0], q[1], q[2], v.paint, shell), v, shell);
}

@vertex
fn vs_shadow(v: In) -> @builtin(position) vec4<f32> {
    return scene.clip * vec4<f32>(place(v.pos, v.normal, v.tangent.xyz, v.paint, 0u).p, 1.0);
}

@vertex
fn vs_shadow_skinned(v: In, b: Bound, @builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let q = posed(v, b, index);
    return scene.clip * vec4<f32>(place(q[0], q[1], q[2], v.paint, 0u).p, 1.0);
}
