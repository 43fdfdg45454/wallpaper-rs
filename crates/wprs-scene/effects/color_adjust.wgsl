fn color_color_adjust(color: vec4<f32>, uv: vec2<f32>) -> vec4<f32> {
    let c = straight(color);
    var rgb = to_linear(c.rgb) * exp2(param(0u).x);
    rgb = 0.18 * pow(max(rgb, vec3<f32>(0.0)) / 0.18, vec3<f32>(param(1u).x));
    rgb = mix(vec3<f32>(dot(rgb, LUMA)), rgb, param(2u).x);
    let lab = oklab(rgb);
    let h = radians(param(3u).x);
    let ab = vec2<f32>(lab.y * cos(h) - lab.z * sin(h), lab.y * sin(h) + lab.z * cos(h));
    rgb = max(from_oklab(vec3<f32>(lab.x, ab)), vec3<f32>(0.0));
    return vec4<f32>(from_linear(rgb) * c.a, c.a);
}
