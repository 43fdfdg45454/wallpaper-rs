struct Fx {
    a: vec4<f32>,
    b: vec4<f32>,
    size: vec2<f32>,
    time: f32,
}

@group(0) @binding(0) var<uniform> fx: Fx;
@group(0) @binding(1) var layer_texture: texture_2d<f32>;
@group(0) @binding(2) var layer_sampler: sampler;

const TAU: f32 = 6.2831853;

fn tap(uv: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(layer_texture, layer_sampler, uv, 0.0);
}

fn fx_blur(uv: vec2<f32>) -> vec4<f32> {
    let n = i32(clamp(fx.a.x * 0.5, 1.0, 4.0));
    let d = fx.a.x / f32(n) / fx.size;
    var sum = vec4<f32>(0.0);
    for (var x = -n; x <= n; x++) {
        for (var y = -n; y <= n; y++) {
            sum += tap(uv + vec2<f32>(f32(x), f32(y)) * d);
        }
    }
    return sum / f32((2 * n + 1) * (2 * n + 1));
}

fn fx_color_grade(uv: vec2<f32>) -> vec4<f32> {
    let c = tap(uv);
    let k = vec3<f32>(0.57735027);
    let h = radians(fx.a.w);
    var rgb = (c.rgb - 0.5) * fx.a.y + 0.5 + fx.a.x;
    rgb = mix(vec3<f32>(dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722))), rgb, fx.a.z);
    rgb = rgb * cos(h) + cross(k, rgb) * sin(h) + k * dot(k, rgb) * (1.0 - cos(h));
    return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), c.a);
}

fn fx_waves(uv: vec2<f32>) -> vec4<f32> {
    let dir = vec2<f32>(cos(radians(fx.a.w)), sin(radians(fx.a.w)));
    let phase = (dot(uv, dir) * fx.a.y - fx.time * fx.a.z) * TAU;
    return tap(uv + vec2<f32>(-dir.y, dir.x) * sin(phase) * fx.a.x / fx.size);
}

fn fx_scroll(uv: vec2<f32>) -> vec4<f32> {
    return tap(fract(uv - fx.a.xy * fx.time / fx.size));
}

fn fx_shake(uv: vec2<f32>) -> vec4<f32> {
    let t = fx.time * fx.a.y * TAU;
    return tap(uv + vec2<f32>(sin(t), cos(t * 1.618)) * fx.a.x / fx.size);
}

fn fx_vignette(uv: vec2<f32>) -> vec4<f32> {
    let c = tap(uv);
    let d = distance(uv, vec2<f32>(0.5)) / max(fx.a.y, 0.01);
    return vec4<f32>(c.rgb * (1.0 - fx.a.x * smoothstep(0.0, 1.0, d)), c.a);
}

fn fx_pulse(uv: vec2<f32>) -> vec4<f32> {
    let c = tap(uv);
    return vec4<f32>(c.rgb * (1.0 + fx.a.x * sin(fx.time * fx.a.y * TAU)), c.a);
}

fn fx_tint(uv: vec2<f32>) -> vec4<f32> {
    let c = tap(uv);
    return vec4<f32>(mix(c.rgb, c.rgb * fx.a.rgb, fx.b.x * fx.a.a), c.a);
}

fn fx_pixelate(uv: vec2<f32>) -> vec4<f32> {
    let cell = max(fx.a.x, 1.0) / fx.size;
    return tap((floor(uv / cell) + 0.5) * cell);
}

fn fx_chromatic_aberration(uv: vec2<f32>) -> vec4<f32> {
    let c = tap(uv);
    let d = (uv - 0.5) * fx.a.x / fx.size;
    return vec4<f32>(tap(uv + d).r, c.g, tap(uv - d).b, c.a);
}
