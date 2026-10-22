struct Frame {
    time: f32,
    dt: f32,
    resolution: vec2<f32>,
    cursor: vec2<f32>,
    cursor_delta: vec2<f32>,
    levels: vec4<f32>,
    beat: f32,
    srgb: f32,
    quality: f32,
    seed: f32,
    audio: array<vec4<f32>, 16>,
};
