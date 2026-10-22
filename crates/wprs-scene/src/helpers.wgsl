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

fn hash(p: vec2<f32>) -> f32 {
    var q = fract(p.xyx * 0.1031);
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn hash2(p: vec2<f32>) -> vec2<f32> {
    var q = fract(p.xyx * vec3<f32>(0.1031, 0.1030, 0.0973));
    q += dot(q, q.yzx + 33.33);
    return fract((q.xx + q.yz) * q.zy);
}

fn noise1(x: f32) -> f32 {
    let i = floor(x);
    let f = fract(x);
    return mix(hash(vec2<f32>(i, 0.0)), hash(vec2<f32>(i + 1.0, 0.0)), f * f * (3.0 - 2.0 * f));
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(hash(i), hash(i + vec2<f32>(1.0, 0.0)), u.x);
    return mix(a, mix(hash(i + vec2<f32>(0.0, 1.0)), hash(i + vec2<f32>(1.0)), u.x), u.y);
}

fn gradient_corner(i: vec2<f32>, f: vec2<f32>, o: vec2<f32>) -> f32 {
    return dot(hash2(i + o) * 2.0 - 1.0, f - o);
}

fn gradient_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let a = mix(gradient_corner(i, f, vec2<f32>(0.0)), gradient_corner(i, f, vec2<f32>(1.0, 0.0)), u.x);
    let b = mix(gradient_corner(i, f, vec2<f32>(0.0, 1.0)), gradient_corner(i, f, vec2<f32>(1.0)), u.x);
    return mix(a, b, u.y) * 1.4142;
}

fn fbm(p: vec2<f32>, octaves: i32) -> f32 {
    var sum = 0.0;
    var amp = 0.5;
    var q = p;
    for (var i = 0; i < min(octaves, 8); i++) {
        sum += amp * gradient_noise(q);
        q = mat2x2<f32>(1.6, 1.2, -1.2, 1.6) * q + 17.3;
        amp *= 0.5;
    }
    return sum;
}

fn bayer(p: vec2<f32>) -> f32 {
    let i = vec2<u32>(vec2<i32>(floor(p)) & vec2<i32>(3));
    let x = i.x ^ i.y;
    return (f32(((x & 1u) << 3u) | ((i.y & 1u) << 2u) | (x & 2u) | (i.y >> 1u)) + 0.5) / 16.0;
}
