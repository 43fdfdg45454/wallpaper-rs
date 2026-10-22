fn gauss(t: texture_2d<f32>, uv: vec2<f32>, dir: vec2<f32>, radius: f32) -> vec4<f32> {
    let r = max(radius * fx.info.w, 0.0);
    if r < 0.5 {
        return sample(t, uv);
    }
    let sigma = max(r / 2.5, 0.5);
    var sum = vec4<f32>(0.0);
    var total = 0.0;
    for (var i = -8; i <= 8; i++) {
        let x = f32(i) * r / 8.0;
        let w = exp(-x * x / (2.0 * sigma * sigma));
        sum += sample(t, uv + dir * x * fx.texel.xy) * w;
        total += w;
    }
    return sum / total;
}

fn oklab(c: vec3<f32>) -> vec3<f32> {
    let l = pow(max(mat3x3<f32>(0.4122214708, 0.2119034982, 0.0883024619, 0.5363325363, 0.6806995451, 0.2817188376,
        0.0514459929, 0.1073969566, 0.6299787005) * c, vec3<f32>(0.0)), vec3<f32>(1.0 / 3.0));
    return mat3x3<f32>(0.2104542553, 1.9779984951, 0.0259040371, 0.7936177850, -2.4285922050, 0.7827717662,
        -0.0040720468, 0.4505937099, -0.8086757660) * l;
}

fn from_oklab(c: vec3<f32>) -> vec3<f32> {
    let l = mat3x3<f32>(1.0, 1.0, 1.0, 0.3963377774, -0.1055613458, -0.0894841775, 0.2158037573, -0.0638541728,
        -1.2914855480) * c;
    return mat3x3<f32>(4.0767416621, -1.2684380046, -0.0041960863, -3.3077115913, 2.6097574011, -0.7034186147,
        0.2309699292, -0.3413193965, 1.7076147010) * (l * l * l);
}

@fragment
fn fs_blur_h(in: VertexOut) -> @location(0) vec4<f32> {
    return gauss(input_tex, in.uv, vec2<f32>(1.0, 0.0), param(0u).x);
}

@fragment
fn fs_blur_v(in: VertexOut) -> @location(0) vec4<f32> {
    return gauss(input_tex, in.uv, vec2<f32>(0.0, 1.0), param(0u).x);
}

@fragment
fn fs_color_adjust(in: VertexOut) -> @location(0) vec4<f32> {
    let c = straight(tap(in.uv));
    var rgb = to_linear(c.rgb) * exp2(param(0u).x);
    rgb = 0.18 * pow(max(rgb, vec3<f32>(0.0)) / 0.18, vec3<f32>(param(1u).x));
    rgb = mix(vec3<f32>(dot(rgb, LUMA)), rgb, param(2u).x);
    let lab = oklab(rgb);
    let h = radians(param(3u).x);
    let ab = vec2<f32>(lab.y * cos(h) - lab.z * sin(h), lab.y * sin(h) + lab.z * cos(h));
    rgb = max(from_oklab(vec3<f32>(lab.x, ab)), vec3<f32>(0.0));
    return vec4<f32>(from_linear(rgb) * c.a, c.a);
}

@fragment
fn fs_waves(in: VertexOut) -> @location(0) vec4<f32> {
    let angle = radians(param(3u).x);
    let dir = vec2<f32>(cos(angle), sin(angle));
    let phase = (dot(in.uv, dir) * param(1u).x - frame.time * param(2u).x) * TAU;
    let shift = vec2<f32>(-dir.y, dir.x) * sin(phase) * px(param(0u).x) * mask_at(in.uv);
    return tap(in.uv + shift);
}

@fragment
fn fs_bloom_bright(in: VertexOut) -> @location(0) vec4<f32> {
    let c = tap(in.uv);
    let l = max(c.r, max(c.g, c.b));
    let t = param(0u).x;
    let k = max(t * param(3u).x, 1e-4);
    let soft = clamp(l - t + k, 0.0, 2.0 * k);
    let gain = max(soft * soft / (4.0 * k), l - t) / max(l, 1e-4);
    return c * max(gain, 0.0);
}

@fragment
fn fs_bloom_h(in: VertexOut) -> @location(0) vec4<f32> {
    return gauss(input_tex, in.uv, vec2<f32>(1.0, 0.0), param(2u).x);
}

@fragment
fn fs_bloom_v(in: VertexOut) -> @location(0) vec4<f32> {
    return gauss(input_tex, in.uv, vec2<f32>(0.0, 1.0), param(2u).x);
}

@fragment
fn fs_bloom_mix(in: VertexOut) -> @location(0) vec4<f32> {
    return tap(in.uv) + sample(aux0, in.uv) * param(1u).x;
}

@fragment
fn fs_mask_mix(in: VertexOut) -> @location(0) vec4<f32> {
    return mix(sample(aux0, in.uv), tap(in.uv), mask_at(in.uv));
}
