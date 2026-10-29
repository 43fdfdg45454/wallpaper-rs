struct Surface {
    position: vec3<f32>,
    normal: vec3<f32>,
    uv: vec2<f32>,
    albedo: vec4<f32>,
    emissive: vec3<f32>,
    roughness: f32,
    metallic: f32,
    occlusion: f32,
};

struct Sprite {
    position: vec2<f32>,
    velocity: vec2<f32>,
    corner: vec2<f32>,
    uv: vec2<f32>,
    color: vec4<f32>,
    size: f32,
    rotation: f32,
    age: f32,
    life: f32,
};
