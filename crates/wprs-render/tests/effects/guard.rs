use super::*;
use std::sync::mpsc::channel;
use std::time::Duration;

const PLAIN: &str = "override taps: u32 = 4u;
@fragment fn fs_endless(in: VertexOut) -> @location(0) vec4<f32> { return tap(in.uv) * f32(taps); }";

fn shader(body: &str) -> String {
    format!(
        "override taps: u32 = 4u;\noverride neg: i32 = -1;\n@fragment\nfn fs_endless(in: VertexOut) -> @location(0) vec4<f32> {{\n    var c = tap(in.uv);\n    var n = 0u;\n{body}\n    return vec4<f32>(c.rgb, 1.0);\n}}"
    )
}

fn endless(params: Value, body: &str) -> Option<[u8; 3]> {
    drawn(params, PLAIN, Some(body))
}

fn drawn(params: Value, checked: &str, body: Option<&str>) -> Option<[u8; 3]> {
    let json = json!({ "id": "endless", "cost": {}, "passes": [{ "entry": "fs_endless" }],
        "options": [{ "id": "taps", "values": [0, 4], "default": 4 }] });
    let (checked, wgsl, body) = (checked.to_string(), body.map(shader), body.unwrap_or_default().to_string());
    let (send, done) = channel();
    std::thread::spawn(move || {
        let fx = Fx::new("endless", params).own(json, &checked).input(solid([255; 3]));
        let fx = match &wgsl {
            Some(wgsl) => fx.unchecked("endless", wgsl),
            None => fx,
        };
        let found = fx.run().map(|mut run| (run.shot().at(32, 16), run.view.left_out()));
        let _ = send.send(found);
    });
    let found = done.recv_timeout(Duration::from_secs(30)).unwrap_or_else(|e| panic!("{body}: {e:?}"))?;
    assert!(found.1.is_empty(), "{body}: {:?}", found.1);
    Some(found.0)
}

#[test]
fn the_injected_guard_alone_ends_loops_that_never_break() {
    let cases = [
        ("loop { n++; if n == 0u { break; } }", "n == 1024u"),
        ("for (;;) { n++; if c.a < -1.0 { break; } }", "n == 1024u"),
        ("while c.a > -1.0 { n++; }", "n == 1024u"),
        ("loop { loop { n++; if n == 0u { break; } } if n == 0u { break; } }", "n > 60000u && n <= 65536u"),
        (
            "for (var i = 0u; i < 2u; i++) { loop { n++; if n == 0u { break; } } loop { n++; if n == 0u { break; } } }",
            "n == 4096u",
        ),
    ];
    for (body, expected) in cases {
        let body = format!("{body}\n    c = select(vec4<f32>(0.0, 0.0, 1.0, 1.0), vec4<f32>(1.0, 0.0, 0.0, 1.0), {expected});");
        let Some(found) = endless(json!({}), &body) else { return };
        assert_eq!(found, RED, "{body}");
    }
}

#[test]
fn loops_the_checker_once_missed_finish_on_the_gpu() {
    let bodies = [
        "for (var i = 4294967290u; i <= 4294967295u; i++) { c *= 0.9; }",
        "for (var i = 2147483640; i <= 2147483647; i++) { c *= 0.9; }",
        "for (var x = 16777216.0; x < 16777300.0; x += 1.0) { c *= 0.9; }",
        "for (var i = 0u; i < 16u; i += taps) { for (var j = 0u; j < 1000u; j += taps) { c *= 0.9; } }",
        "let m = -1i; for (var i = 0u; i <= u32(m); i++) { c *= 0.9; }",
        "for (var i = 0u; i <= u32(neg); i++) { c *= 0.9; }",
        "for (var i: u32 = u32(neg); i > 0u; i--) { c *= 0.9; }",
        "for (var i: u32 = bitcast<u32>(neg); i > 0u; i--) { c *= 0.9; }",
        "for (var i = 0u; i <= bitcast<u32>(neg); i++) { c *= 0.9; }",
        "for (var i = 4294967000u; i < 4294967295u; i += bitcast<u32>(0.0f)) { c *= 0.9; }",
        "for (var i = 5; u32(i) >= 0u; i--) { c *= 0.9; }",
        "for (var i = 10u; i >= 0u; i--) { c *= 0.9; }",
    ];
    for body in bodies {
        let Some(found) = endless(json!({ "taps": 0 }), body) else { return };
        assert_eq!(found, [0; 3], "{body}");
    }
}

#[test]
fn variables_started_from_a_bitcast_override_draw_instead_of_crashing_the_driver() {
    let body = "var k: u32 = bitcast<u32>(neg);\n    var f = bitcast<f32>(taps | 1065353216u);\n    c = select(vec4<f32>(0.0, 0.0, 1.0, 1.0), vec4<f32>(1.0, 0.0, 0.0, 1.0), k == 4294967295u && f > 0.5);";
    let Some(found) = drawn(json!({}), &shader(body), None) else { return };
    assert_eq!(found, RED);
    let bounded = "for (var i: u32 = bitcast<u32>(neg); i > 4294967200u; i--) { c *= 0.9; }";
    let Some(found) = endless(json!({}), bounded) else { return };
    assert_eq!(found, [0; 3]);
}
