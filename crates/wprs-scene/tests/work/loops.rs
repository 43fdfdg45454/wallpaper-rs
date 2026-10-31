use super::*;

#[test]
fn counted_loops_pass_and_unbounded_work_is_refused() {
    let fine = [
        "for (var i = 0u; i < 8u; i++) { c += tap(in.uv + f32(i)); }",
        "for (var i = 0; i <= 1023; i += 1) { c *= 0.99; }",
        "for (var i = 0u; i < taps; i++) { c *= 0.9; }",
        "for (var i = 0u; i < min(u32(param(0u).x), 64u); i++) { c *= 0.9; }",
        "for (var i = 16; i > 0; i--) { c *= 0.9; }",
        "for (var i = 0u; i < 32u; i++) { for (var j = 0u; j < 32u; j++) { c *= 0.99; } }",
        "for (var i = 10u; i >= 2u; i -= 2u) { c *= 0.9; }",
        "for (var i = 4294967290u; i < 4294967295u; i++) { c *= 0.9; }",
        "let m = 8i; for (var i = 0u; i < u32(m); i++) { c *= 0.9; }",
        "for (var i = 0u; i < min(u32(neg), 64u); i++) { c *= 0.9; }",
    ];
    for body in fine {
        effect(|_| (), body).unwrap_or_else(|e| panic!("{body}: {e}"));
    }
    let refused = [
        ("var i = 0u; loop { if i > 4u { break; } i++; }", "a counter that a literal limits"),
        ("loop { if c.a < -1.0 { break; } }", "a counter that a literal limits"),
        ("for (var i = 0u; i < u32(param(0u).x); i++) { c *= 0.9; }", "a counter that a literal limits"),
        ("for (var i = 0u; i < 8u; i++) { i = 0u; }", "a counter that a literal limits"),
        ("for (var i = 0u; i < 4096u; i++) { c *= 0.9; }", "a counter that a literal limits"),
        ("var i = 0u; loop { i++; continuing { break if i > 4u; } }", "a counter that a literal limits"),
        ("for (var i = 10u; i >= 0u; i--) { c *= 0.9; }", "a counter that a literal limits"),
        ("for (var i = 10u; i > 0u; i -= 3u) { c *= 0.9; }", "a counter that a literal limits"),
        (
            "for (var x = 16777216.0; x < 16777300.0; x += 1.0) { c *= 0.9; }",
            "a counter that a literal limits",
        ),
        (
            "for (var i = 0u; i < 16u; i += min(u32(param(0u).x), 8u)) { c *= 0.9; }",
            "a counter that a literal limits",
        ),
        ("for (var i = 0u; i < 16u; i += taps) { c *= 0.9; }", "a counter that a literal limits"),
        ("for (var i = 2147483640; i <= 2147483647; i++) { c *= 0.9; }", "a counter that a literal limits"),
        ("for (var i = 4294967290u; i <= 4294967295u; i++) { c *= 0.9; }", "a counter that a literal limits"),
        ("for (var i = 5; u32(i) >= 0u; i--) { c *= 0.9; }", "a counter that a literal limits"),
        ("for (var i = 0u; i < bitcast<u32>(1.0f); i++) { c *= 0.9; }", "a counter that a literal limits"),
        (
            "for (var i = 0u; i < 16u; i += bitcast<u32>(1.0f)) { c *= 0.9; }",
            "a counter that a literal limits",
        ),
        (
            "for (var i = 4294967000u; i < 4294967295u; i += bitcast<u32>(1.0f)) { c *= 0.9; }",
            "a counter that a literal limits",
        ),
        ("let m = -1i; for (var i = 0u; i <= u32(m); i++) { c *= 0.9; }", "a counter that a literal limits"),
        ("for (var i = 0u; i <= u32(neg); i++) { c *= 0.9; }", "a counter that a literal limits"),
        ("for (var i = 0u; i <= bitcast<u32>(neg); i++) { c *= 0.9; }", "a counter that a literal limits"),
        ("for (var i: u32 = u32(neg); i > 0u; i--) { c *= 0.9; }", "a counter that a literal limits"),
        (
            "for (var i: u32 = bitcast<u32>(neg); i > 0u; i--) { c *= 0.9; }",
            "a counter that a literal limits",
        ),
        (
            "let m = 4294967200u; for (var i = 4294967290u; i <= u32(f32(m)); i++) { c *= 0.9; }",
            "a counter that a literal limits",
        ),
        ("var k = 0u; helper(&k); c *= f32(k);", "pointer to a local variable"),
        (
            "for (var i = 0u; i < 512u; i++) { for (var j = 0u; j < 512u; j++) { c *= 0.99; } }",
            "operations a pixel",
        ),
    ];
    for (body, problem) in refused {
        let fault = effect(|_| (), body).err().unwrap_or_else(|| panic!("{body} was accepted"));
        assert!(fault.contains(problem), "{body}: {fault}");
    }
    let wide = effect(
        |d| d["options"][0]["values"] = json!([4, 2048]),
        "for (var i = 0u; i < taps; i++) { c *= 0.9; }",
    );
    assert!(wide.unwrap_err().contains("a counter that a literal limits"));
    let negative = |d: &mut Json| {
        d["options"].as_array_mut().unwrap().push(json!({ "id": "neg", "values": [-1, -2], "default": -1 }))
    };
    for body in [
        "for (var i: u32 = u32(neg); i > 0u; i--) { c *= 0.9; }",
        "for (var i = 0u; i <= u32(neg); i++) { c *= 0.9; }",
    ] {
        assert!(effect(negative, body).unwrap_err().contains("a counter that a literal limits"), "{body}");
    }
    let huge = effect(|d| d["options"][0]["values"] = json!([4, -1]), "");
    assert!(huge.unwrap_err().contains("values its override cannot hold"));
    let stepped = effect(
        |d| d["options"][0]["values"] = json!([0, 4]),
        "for (var i = 0u; i < 16u; i += taps) { for (var j = 0u; j < 1000u; j += taps) { c *= 0.9; } }",
    );
    assert!(stepped.unwrap_err().contains("a counter that a literal limits"));
}
