fn body_e(in: VertexOut) -> vec4<f32> { var s = vec4<f32>(0.0); for (var i = 0; i < 128; i++) { let a = f32(i) * 2.39996; let r = sqrt(f32(i) / 128.0); s += tap(in.uv + vec2<f32>(cos(a), sin(a)) * r * 0.01); } return s / 128.0; }
@fragment fn fs_e(in: VertexOut) -> @location(0) vec4<f32> { return body_e(in) + vec4<f32>(frame.time * 1e-9); }
