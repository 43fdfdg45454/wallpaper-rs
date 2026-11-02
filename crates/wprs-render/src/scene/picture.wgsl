fn wrapped(u: vec2<f32>) -> vec2<f32> {
    if item.shape.x > 2.5 {
        return 1.0 - abs(1.0 - (u - 2.0 * floor(u * 0.5)));
    }
    if item.shape.x > 1.5 {
        return fract(u);
    }
    return clamp(u, vec2<f32>(0.0), vec2<f32>(1.0));
}

fn picture_at(in: Out) -> vec2<f32> {
    let room = 0.5 / max(vec2<f32>(textureDimensions(src_tex)) * item.uv.zw, vec2<f32>(1.0));
    return item.uv.xy + clamp(wrapped(in.uv * item.shape.zw), room, 1.0 - room) * item.uv.zw;
}

@fragment
fn fs_picture(in: Out) -> @location(0) vec4<f32> {
    return finish(picture(in, picture_at(in)), in);
}

fn picture(in: Out, at: vec2<f32>) -> vec4<f32> {
    let tiles = in.uv * item.shape.zw;
    let dx = dpdx(tiles * item.uv.zw);
    let dy = dpdy(tiles * item.uv.zw);
    var t = textureSampleGrad(src_tex, samp, at, dx, dy);
    if item.shape.y > 0.5 {
        let size = vec2<f32>(textureDimensions(src_tex));
        t = textureLoad(src_tex, vec2<i32>(min(floor(at * size), size - 1.0)), 0);
    }
    return styled(t) * item.color;
}
