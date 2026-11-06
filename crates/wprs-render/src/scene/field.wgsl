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
const TOP: f32 = 65535.0;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> Out {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return Out(vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0));
}

fn alpha(p: vec2<i32>) -> f32 {
    let size = vec2<i32>(textureDimensions(cover));
    return textureLoad(cover, clamp(p, vec2<i32>(0), size - 1), 0).a;
}

fn word(v: f32) -> vec2<f32> {
    let n = round(clamp(v, 0.0, TOP));
    return vec2<f32>(floor(n / 256.0), n - floor(n / 256.0) * 256.0) / 255.0;
}

fn unword(b: vec2<f32>) -> f32 {
    let n = round(b * 255.0);
    return n.x * 256.0 + n.y;
}

fn packed(off: vec2<f32>) -> vec4<f32> {
    if off.x >= FAR * 0.5 {
        return vec4<f32>(1.0);
    }
    let q = (clamp(off / jump.step.y, vec2<f32>(-1.0), vec2<f32>(1.0)) * 0.5 + 0.5) * (TOP - 1.0);
    return vec4<f32>(word(q.x), word(q.y));
}

fn offset_at(p: vec2<i32>) -> vec2<f32> {
    let t = textureLoad(seeds, p, 0);
    let q = vec2<f32>(unword(t.xy), unword(t.zw));
    if q.x > TOP - 0.5 {
        return vec2<f32>(FAR);
    }
    return (q / (TOP - 1.0) * 2.0 - 1.0) * jump.step.y;
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
        return packed(vec2<f32>(FAR));
    }
    let g = vec2<f32>(alpha(p + sides[0]) - alpha(p + sides[1]), alpha(p + sides[2]) - alpha(p + sides[3])) * 0.5;
    let n = dot(g, g);
    let off = select(vec2<f32>(0.0), g * (0.5 - c) / max(n, 1e-6), n > 1e-6);
    return packed(clamp(off, vec2<f32>(-1.0), vec2<f32>(1.0)));
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
            let off = offset_at(q);
            let to = vec2<f32>(q - p) + off;
            if off.x < FAR * 0.5 && dot(to, to) < near && all(abs(to) <= vec2<f32>(jump.step.y)) {
                near = dot(to, to);
                best = to;
            }
        }
    }
    return packed(best);
}

@fragment
fn fs_field(in: Out) -> @location(0) vec4<f32> {
    let p = vec2<i32>(in.position.xy);
    let c = alpha(p);
    let to = offset_at(p);
    let d = select(length(to), FAR, to.x >= FAR * 0.5);
    let signed = clamp(select(d, -d, c >= 0.5) / jump.step.y, -1.0, 1.0);
    return vec4<f32>(c, word((signed * 0.5 + 0.5) * TOP), 1.0);
}
