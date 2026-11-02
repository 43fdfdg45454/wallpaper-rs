struct Look {
    m0: vec4<f32>,
    m1: vec4<f32>,
    tint: vec4<f32>,
    atlas: vec4<f32>,
    trail: vec4<f32>,
    shade: vec4<f32>,
    extra: vec4<f32>,
    cut: vec4<f32>,
    turn: vec4<f32>,
    deep: vec4<f32>,
    strand: vec4<f32>,
    cord: vec4<f32>,
    light: vec4<f32>,
    place: vec4<f32>,
    bend: vec4<f32>,
    sizes: array<vec4<f32>, 16>,
    colors: array<vec4<f32>, 64>,
};

struct Corner {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) frames: vec3<f32>,
    @location(3) @interpolate(flat) turn: vec4<f32>,
};

@group(0) @binding(0) var<uniform> look: Look;
@group(0) @binding(1) var samp: sampler;
@group(1) @binding(0) var sprite: texture_2d<f32>;
@group(1) @binding(1) var<uniform> rects: array<vec4<f32>, 256>;

fn curve(t: f32) -> f32 {
    let x = t * 63.0;
    let i = min(u32(x), 62u);
    let j = i + 1u;
    return mix(look.sizes[i / 4u][i % 4u], look.sizes[j / 4u][j % 4u], x - f32(i));
}

fn decode(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
}

fn encode(c: vec3<f32>) -> vec3<f32> {
    return select(1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3<f32>(0.0031308));
}

@vertex
fn vs_particle(
    @builtin(vertex_index) v: u32,
    @location(0) a: vec4<f32>,
    @location(1) b: vec4<f32>,
    @location(2) c: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(4) e: vec4<f32>,
    @location(5) d: vec4<f32>,
) -> Corner {
    var out: Corner;
    out.position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    let far = look.deep.x + d.x;
    if b.x >= b.y || (look.deep.w > 0.5 && far <= 1.0) {
        return out;
    }
    let t = clamp(b.x / b.y, 0.0, 1.0);
    let k = select(1.0, look.deep.x / far, look.deep.w > 0.5);
    let center = look.deep.yz + (a.xy - look.deep.yz) * k;
    let size = b.z * curve(t) * look.tint.z * e.w * k;
    let corner = vec2<f32>(f32(v & 1u), f32(v >> 1u));
    let speed = length(a.zw);
    let turned = look.shade.z > 0.0 && look.shade.z == look.shade.w;
    let tail = select(clamp(look.trail.x * speed, look.shade.z, look.shade.w), 0.0, turned || speed <= 0.0);
    var axis = vec2<f32>(cos(radians(b.w)), sin(radians(b.w)));
    if (tail > 0.0 || turned || look.turn.x == 1.0) && speed > 0.0 {
        axis = a.zw / speed;
    }
    if look.turn.x == 2.0 {
        axis = vec2<f32>(cos(look.turn.y - look.turn.z), sin(look.turn.y - look.turn.z));
    }
    let x = t * 63.0;
    let i = min(u32(x), 62u);
    let tone = mix(look.colors[i], look.colors[i + 1u], x - f32(i));
    let base = select(vec4<f32>(decode(color.rgb), color.a), color, look.tint.y > 0.5);
    let shade = base * tone * vec4<f32>(vec3<f32>(look.shade.x), look.tint.x * e.z);
    let s = particle(Sprite(center, a.zw, corner, corner, shade, size, degrees(atan2(axis.y, axis.x)), b.x, b.y));
    let turn = vec2<f32>(cos(radians(s.rotation)), sin(radians(s.rotation)));
    let along = mix(-0.5 * s.size - tail, 0.5 * s.size, s.corner.x);
    let p = vec3<f32>(s.position + turn * along + vec2<f32>(-turn.y, turn.x) * (s.corner.y - 0.5) * s.size, 1.0);
    out.position = vec4<f32>(dot(look.m0.xyz, p), dot(look.m1.xyz, p), 0.0, 1.0);
    out.uv = s.uv;
    out.color = s.color;
    let spun = radians(s.rotation) + look.turn.z;
    out.turn = vec4<f32>(cos(spun), sin(spun), s.size * look.light.w, 0.0);
    let n = max(look.atlas.x * look.atlas.y, 1.0);
    var f = c.y;
    switch u32(look.atlas.z) {
        case 1u: {
            f = c.y + b.x * look.atlas.w;
        }
        case 2u: {
            f = select(min(c.y + t * n, n - 0.001), c.y + fract(t * look.shade.y) * n, look.shade.y > 1.0);
        }
        case 3u: {
            f = floor(c.z * n);
        }
        default: {}
    }
    out.frames = vec3<f32>(floor(f) % n, (floor(f) + 1.0) % n, fract(f) * look.trail.w);
    return out;
}

fn cell(f: f32, uv: vec2<f32>) -> vec4<f32> {
    if look.extra.x > 0.5 {
        let r = rects[min(u32(f), 255u)];
        let edge = min(0.5 / vec2<f32>(textureDimensions(sprite)), r.zw * 0.5);
        return textureSample(sprite, samp, r.xy + clamp(uv * r.zw, edge, r.zw - edge));
    }
    let cols = max(look.atlas.x, 1.0);
    let at = vec2<f32>(f % cols, floor(f / cols));
    return textureSample(sprite, samp, (at + uv) / vec2<f32>(cols, max(look.atlas.y, 1.0)));
}

fn painted(in: Corner) -> vec4<f32> {
    var s = mix(cell(in.frames.x, in.uv), cell(in.frames.y, in.uv), in.frames.z);
    if look.tint.y > 0.5 {
        s = vec4<f32>(encode(s.rgb), s.a);
    }
    let disc = vec4<f32>(1.0, 1.0, 1.0, 1.0 - smoothstep(0.5, 1.0, length(in.uv * 2.0 - 1.0)));
    var c = select(disc, s, look.tint.w > 0.5);
    if look.cut.w > 0.5 {
        c.a = clamp((c.a - look.cut.x) / (look.cut.y - look.cut.x), 0.0, 1.0) * look.cut.z;
    }
    c *= in.color;
    return vec4<f32>(c.rgb * c.a, c.a);
}

@fragment
fn fs_particle(in: Corner) -> @location(0) vec4<f32> {
    return painted(in);
}
