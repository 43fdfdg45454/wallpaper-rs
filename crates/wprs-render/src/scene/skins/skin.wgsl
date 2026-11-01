struct Bones { m: array<vec4<f32>, 768> };
struct Pieces { box_size: vec4<f32>, lift: vec4<f32>, data: array<vec4<f32>, 384> };
struct In {
    @builtin(vertex_index) index: u32,
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) joints: vec4<f32>,
    @location(3) weights: vec4<f32>,
    @location(4) piece: f32,
};
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) color: vec4<f32> };

@group(0) @binding(0) var<uniform> bones: Bones;
@group(0) @binding(1) var<uniform> pieces: Pieces;
@group(0) @binding(2) var morphs: texture_2d<f32>;
@group(0) @binding(3) var picture: texture_2d<f32>;
@group(0) @binding(4) var smp: sampler;

fn delta(at: u32) -> vec3<f32> {
    return textureLoad(morphs, vec2<i32>(i32(at % 2048u), i32(at / 2048u)), 0).xyz;
}

@vertex
fn vs_skin(v: In) -> Out {
    let k = u32(v.piece) * 6u;
    let local = v.index - u32(pieces.data[k].x);
    var p = v.pos;
    for (var s = 1u; s < 5u; s++) {
        let pair = pieces.data[k + s];
        if pair.y != 0.0 {
            p += delta(u32(pair.x) + local) * pair.y;
        }
        if pair.w != 0.0 {
            p += delta(u32(pair.z) + local) * pair.w;
        }
    }
    let at = vec4<f32>(p, 1.0);
    var moved = vec3<f32>(0.0);
    for (var j = 0u; j < 4u; j++) {
        let b = u32(v.joints[j]) * 3u;
        moved += vec3<f32>(dot(bones.m[b], at), dot(bones.m[b + 1u], at), dot(bones.m[b + 2u], at)) * v.weights[j];
    }
    let lift = pieces.lift.xyz * moved.z;
    let ndc = (moved.xy + lift.xy) / max(1.0 + lift.z, 0.05) / (pieces.box_size.xy * 0.5);
    let depth = clamp(0.5 - moved.z * pieces.box_size.z, 0.0, 1.0);
    return Out(vec4<f32>(ndc, depth, 1.0), v.uv, pieces.data[k + 5u]);
}

@fragment
fn fs_skin(in: Out) -> @location(0) vec4<f32> {
    let c = textureSample(picture, smp, in.uv) * in.color;
    if c.a < 0.001 {
        discard;
    }
    return vec4<f32>(c.rgb * c.a, c.a);
}
