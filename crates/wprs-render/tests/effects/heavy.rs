use super::*;
use std::time::{Duration, Instant};
use wprs_scene::{MAX_INLINED, MAX_MEMORY, MAX_OPS};

fn inlined(statements: usize, calls: usize) -> String {
    let mut s = String::from("fn f0(m: mat4x4<f32>) -> mat4x4<f32> { var a = m; let n = m * 0.2; ");
    s.push_str(&"a = a * n;".repeat(statements));
    s.push_str(" return a; }\nfn f1(m: mat4x4<f32>) -> mat4x4<f32> { var a = m; ");
    s.push_str(&"a = f0(a);".repeat(calls));
    s + " return a; }\n@fragment fn fs_h(in: VertexOut) -> @location(0) vec4<f32> {
        let v = tap(in.uv); return v + f1(mat4x4<f32>(v, v, v, v))[0] * 1e-30; }"
}

fn looped(statements: usize) -> String {
    format!(
        "@fragment fn fs_h(in: VertexOut) -> @location(0) vec4<f32> {{
            let v = tap(in.uv); var m = mat4x4<f32>(v, v.yxzw, v.zwxy, v.wzyx); let n = m * 0.2;
            for (var j = 0u; j < 1024u; j++) {{ {} }}
            return v + m[0] * 1e-30; }}",
        "m = m * n;".repeat(statements)
    )
}

fn kept(vectors: usize) -> String {
    format!(
        "@fragment fn fs_h(in: VertexOut) -> @location(0) vec4<f32> {{
            var a: array<vec4<f32>, {vectors}>; let i = u32(in.position.x) % {vectors}u;
            a[i] = tap(in.uv); return a[i]; }}"
    )
}

fn scattered(n: usize, rounds: usize) -> String {
    let m = n - 1;
    format!(
        "@fragment fn fs_h(in: VertexOut) -> @location(0) vec4<f32> {{
            let v = tap(in.uv); var r: array<vec4<f32>, {n}>;
            var k = u32(in.position.x) * 7919u + u32(in.position.y) * 104729u;
            for (var i = 0u; i < {rounds}u; i++) {{ for (var j = 0u; j < 1u; j++) {{
                k = k * 1664525u + 1013904223u; r[k & {m}u] = r[(k >> 9u) & {m}u] + v; }} }}
            return v + r[k & {m}u] * 1e-30; }}"
    )
}

fn live(arrays: usize) -> String {
    let lets: String = (0..arrays).map(|a| format!("let a{a} = mk(in.uv, {a}.0);")).collect();
    let sums: String = (0..arrays).map(|a| format!(" + a{a}[(u32(in.position.x) + {a}u) % 64u]")).collect();
    format!(
        "fn mk(u: vec2<f32>, q: f32) -> array<vec4<f32>, 64> {{
            var r = array<vec4<f32>, 64>(); r[0] = vec4<f32>(u, q, 1.0); r[63] = r[0] * 2.0; return r; }}
        @fragment fn fs_h(in: VertexOut) -> @location(0) vec4<f32> {{
            {lets} return tap(in.uv) + (vec4<f32>(0.0){sums}) * 1e-30; }}"
    )
}

#[test]
fn package_effects_at_the_limits_build_and_draw_in_time() {
    let json = json!({ "id": "heavy", "cost": {}, "passes": [{ "entry": "fs_h" }] });
    let cases = [
        (inlined(40, 8), "inlined"),
        (looped(10), "looped"),
        (kept(MAX_MEMORY as usize / 16 - 64), "kept"),
        (scattered(16, 250), "scattered"),
        (scattered(32, 62), "scattered wide"),
        (live(4), "live"),
    ];
    let fast = Renderer::shared().is_ok_and(|r| !r.software());
    for (wgsl, name) in cases {
        let d =
            Descriptor::packaged("heavy", &json.to_string(), &wgsl).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(d.size <= MAX_INLINED && d.ops <= MAX_OPS as f32, "{name}: {} {}", d.size, d.ops);
        assert!(!name.starts_with("scattered") || d.ops * 2.0 > MAX_OPS as f32, "{name}: {}", d.ops);
        let started = Instant::now();
        let Some(mut run) = Fx::new("heavy", json!({})).own(json.clone(), &wgsl).input(solid(RED)).run()
        else {
            return;
        };
        let built = started.elapsed();
        assert!(run.view.left_out().is_empty(), "{name}: {:?}", run.view.left_out());
        let started = Instant::now();
        let shot = run.shot();
        eprintln!(
            "{name}: {} operations, {} long, built in {built:?}, drawn in {:?}",
            d.ops,
            d.size,
            started.elapsed()
        );
        assert!(built < Duration::from_secs(10) && started.elapsed() < Duration::from_secs(10), "{name}");
        shot.check(&[((32, 16), RED)]);
        if fast {
            let started = Instant::now();
            run.shot_at((1920, 1080)).check(&[((960, 540), RED)]);
            eprintln!("{name}: drawn at 1920x1080 in {:?}", started.elapsed());
            assert!(started.elapsed() < Duration::from_millis(500), "{name}: {:?}", started.elapsed());
        }
    }
}
