fn aces(x: vec3<f32>) -> vec3<f32> {
    let into = mat3x3<f32>(0.59719, 0.07600, 0.02840, 0.35458, 0.90834, 0.13383, 0.04823, 0.01566, 0.83777);
    let back = mat3x3<f32>(1.60475, -0.10208, -0.00327, -0.53108, 1.10813, -0.07276, -0.07367, -0.00605, 1.07602);
    let v = into * x;
    let a = v * (v + 0.0245786) - 0.000090537;
    let b = v * (0.983729 * v + 0.4329510) + 0.238081;
    return clamp(back * (a / b), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn agx(x: vec3<f32>) -> vec3<f32> {
    let into = mat3x3<f32>(0.842479, 0.0423282, 0.0423757, 0.0784336, 0.878469, 0.0784336, 0.0792237, 0.0791661, 0.879143);
    let back = mat3x3<f32>(1.196879, -0.0528969, -0.0529716, -0.0980209, 1.151903, -0.0980435, -0.0990297, -0.0989612, 1.151074);
    let lo = -12.47393;
    let hi = 4.026069;
    var v = clamp(log2(max(into * x, vec3<f32>(1e-10))), vec3<f32>(lo), vec3<f32>(hi));
    v = (v - lo) / (hi - lo);
    let v2 = v * v;
    let v4 = v2 * v2;
    v = 15.5 * v4 * v2 - 40.14 * v4 * v + 31.96 * v4 - 6.868 * v2 * v + 0.4298 * v2 + 0.1191 * v - 0.00232;
    return clamp(pow(max(back * v, vec3<f32>(0.0)), vec3<f32>(2.2)), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_tonemap(in: Out) -> @location(0) vec4<f32> {
    var c = textureSample(src_tex, samp, item.uv.xy + in.uv * item.uv.zw);
    let a = max(c.a, 1e-5);
    var rgb = c.rgb / a;
    if item.mode.y > 0.5 {
        rgb = decode(rgb);
    }
    if in.uv.x < item.mode.w {
        return vec4<f32>(rgb * c.a, c.a);
    }
    if item.mode.z > 1.5 {
        rgb = agx(rgb);
    } else if item.mode.z > 0.5 {
        rgb = aces(rgb);
    }
    return vec4<f32>(rgb * c.a, c.a);
}
