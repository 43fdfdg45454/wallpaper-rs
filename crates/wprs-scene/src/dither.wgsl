fn bayer(p: vec2<f32>) -> f32 {
    let i = vec2<u32>(vec2<i32>(floor(p)) & vec2<i32>(3));
    let x = i.x ^ i.y;
    return (f32(((x & 1u) << 3u) | ((i.y & 1u) << 2u) | (x & 2u) | (i.y >> 1u)) + 0.5) / 16.0;
}
