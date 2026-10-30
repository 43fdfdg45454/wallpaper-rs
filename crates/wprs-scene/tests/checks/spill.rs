use super::*;

#[test]
fn variables_have_a_small_budget_a_pixel() {
    let kept = format!("keeps more than {} KiB in variables a pixel", MAX_MEMORY >> 10);
    let big = [
        entry(
            "var a: array<vec4<f32>, 1000000>; let i = u32(in.position.x); a[i] = tap(in.uv); return a[i / 2u];",
        ),
        entry(
            "var a: array<vec4<f32>, 16384>; let i = u32(in.position.x); a[i] = tap(in.uv); return a[(i * 7u) % 16384u];",
        ),
        format!(
            "var<private> a: array<vec4<f32>, 4096>;\n{}",
            entry("let i = u32(in.position.x); a[i] = tap(in.uv); return a[i / 2u];")
        ),
        entry("let a = array<vec4<f32>, 2048>(); return a[u32(in.position.x)];"),
        format!(
            "fn g(i: u32) -> vec4<f32> {{ var b: array<vec4<f32>, 600>; b[i] = vec4<f32>(1.0); return b[i / 2u]; }}\n{}",
            entry("var a: array<vec4<f32>, 600>; let i = u32(in.position.x); a[i] = g(i); return a[i / 3u];")
        ),
    ];
    big.into_iter().for_each(|w| refused(w, &kept));
    let small = entry(
        "var a: array<vec4<f32>, 64>; let i = u32(in.position.x) % 64u; a[i] = tap(in.uv); return a[(i * 7u) % 64u];",
    );
    pack(&small).unwrap();
    for (n, k) in [(64, 24), (320, 6), (320, 12)] {
        refused(live(n, k), &kept);
    }
    pack(&live(4, 4)).unwrap();
}

fn live(n: usize, k: usize) -> String {
    let lets: String = (0..k).map(|a| format!("let a{a} = mk(in.uv, {a}.0);")).collect();
    let sums: String = (0..k).map(|a| format!("s += a{a}[(j + {a}u) % {n}u];")).collect();
    format!(
        "fn mk(u: vec2<f32>, q: f32) -> array<vec4<f32>, {n}> {{
            var r: array<vec4<f32>, {n}>;
            for (var t = 0u; t < {n}u; t++) {{ r[t] = vec4<f32>(u, q, f32(t)); }}
            return r; }}\n{}",
        entry(&format!(
            "{lets} var s = vec4<f32>(0.0); for (var j = 0u; j < {n}u; j++) {{ {sums} }} return s;"
        ))
    )
}

fn scattered(n: usize, rounds: usize) -> String {
    let m = n - 1;
    entry(&format!(
        "let v = tap(in.uv); var r: array<vec4<f32>, {n}>; var k = u32(in.position.x);
         for (var i = 0u; i < 1024u; i++) {{ for (var j = 0u; j < {rounds}u; j++) {{
             k = k * 1664525u + 1013904223u; r[k & {m}u] = r[(k >> 9u) & {m}u] + v; }} }}
         return r[k & {m}u];"
    ))
}

#[test]
fn computed_indices_into_local_arrays_cost_by_the_size_of_the_arrays() {
    for rounds in [1, 2, 4, 8] {
        refused(scattered(512, rounds), "a pixel");
        refused(scattered(64, rounds), &format!("runs more than {MAX_OPS} operations a pixel"));
    }
    let split = entry(
        "var a: array<vec4<f32>, 32>; var b: array<vec4<f32>, 32>; var k = u32(in.position.x);
         for (var i = 0u; i < 64u; i++) { k = k * 1664525u + 1013904223u; a[k & 31u] = b[(k >> 9u) & 31u]; b[(k >> 4u) & 31u] = a[k & 31u]; }
         return a[k & 31u];",
    );
    refused(split, &format!("runs more than {MAX_OPS} operations a pixel"));
    let d = pack(&scattered(4, 1)).unwrap();
    assert!(d.ops > 1024.0 * 2.0 * 128.0 && d.ops < MAX_OPS as f32, "{}", d.ops);
    let fixed = entry("var a: array<vec4<f32>, 64>; a[3] = tap(in.uv); return a[3] + a[(2u + 1u) % 64u];");
    assert!(pack(&fixed).unwrap().ops < 1000.0);
    let passed = format!(
        "fn g(p: ptr<function, vec4<f32>>) -> vec4<f32> {{ return *p; }}\n{}",
        entry("var a: array<vec4<f32>, 64>; let i = u32(in.position.x) % 64u; return g(&a[i]);")
    );
    refused(passed, "pointer to a local variable");
}
