struct Jump {
    step: vec4<f32>,
};

struct Out {
    @builtin(position) position: vec4<f32>,
};

@group(0) @binding(0) var cover: texture_2d<f32>;
@group(0) @binding(1) var seeds: texture_2d<f32>;
@group(0) @binding(2) var<uniform> jump: Jump;

const FAR: f32 = 30000.0;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> Out {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return Out(vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0));
}

fn alpha(p: vec2<i32>) -> f32 {
    let size = vec2<i32>(textureDimensions(cover));
    return textureLoad(cover, clamp(p, vec2<i32>(0), size - 1), 0).a;
}

@fragment
fn fs_seed(in: Out) -> @location(0) vec4<f32> {
    let p = vec2<i32>(in.position.xy);
    let c = alpha(p);
    let inside = c >= 0.5;
    let sides = array<vec2<i32>, 4>(vec2<i32>(1, 0), vec2<i32>(-1, 0), vec2<i32>(0, 1), vec2<i32>(0, -1));
    var edge = c > 0.0 && c < 1.0;
    for (var k = 0u; k < 4u; k++) {
        edge = edge || (alpha(p + sides[k]) >= 0.5) != inside;
    }
    if !edge {
        return vec4<f32>(FAR, FAR, 0.0, 1.0);
    }
    let g = vec2<f32>(alpha(p + sides[0]) - alpha(p + sides[1]), alpha(p + sides[2]) - alpha(p + sides[3])) * 0.5;
    let n = dot(g, g);
    let off = select(vec2<f32>(0.0), g * (0.5 - c) / max(n, 1e-6), n > 1e-6);
    return vec4<f32>(clamp(off, vec2<f32>(-1.0), vec2<f32>(1.0)), 0.0, 1.0);
}

@fragment
fn fs_jump(in: Out) -> @location(0) vec4<f32> {
    let p = vec2<i32>(in.position.xy);
    let size = vec2<i32>(textureDimensions(seeds));
    let k = i32(jump.step.x);
    var best = vec2<f32>(FAR);
    var near = 1e20;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let q = p + vec2<i32>(x, y) * k;
            if any(q < vec2<i32>(0)) || any(q >= size) {
                continue;
            }
            let off = textureLoad(seeds, q, 0).xy;
            let to = vec2<f32>(q - p) + off;
            if off.x < FAR * 0.5 && dot(to, to) < near {
                near = dot(to, to);
                best = to;
            }
        }
    }
    return vec4<f32>(best, 0.0, 1.0);
}

@fragment
fn fs_field(in: Out) -> @location(0) vec4<f32> {
    let p = vec2<i32>(in.position.xy);
    let c = alpha(p);
    let to = textureLoad(seeds, p, 0).xy;
    let d = select(length(to), FAR, to.x >= FAR * 0.5);
    return vec4<f32>(c, select(d, -d, c >= 0.5), 0.0, 1.0);
}
