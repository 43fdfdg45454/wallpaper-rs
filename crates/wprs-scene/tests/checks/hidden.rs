use super::*;

fn heavy(hidden: bool) -> String {
    let step = "a = fract(sin(a) * 43758.547 + a * 1.37 + 0.11); ".repeat(150);
    let call = |c: &str, x: &str| match hidden {
        true => format!(
            "var i = 0u; loop {{ {c}({x}); if i < 0u {{ }} else {{ break; }} continuing {{ i = i + 1u; }} }}"
        ),
        false => format!("{c}({x});"),
    };
    format!(
        "var<private> G: vec4<f32>;
         fn h0(x: f32) -> f32 {{ var a = x; for (var i = 0u; i < 1000u; i++) {{ {step} }} return a; }}
         fn g0(x: f32) {{ G += vec4<f32>(h0(x)); }}
         fn g1(x: f32) {{ for (var k = 0u; k < 64u; k++) {{ {} }} }}
         @fragment fn fs_p(in: VertexOut) -> @location(0) vec4<f32> {{
           for (var k = 0u; k < 1u; k++) {{ {} }}
           return vec4<f32>(fract(G.x * 0.618), tap(in.uv).yzw);
         }}",
        call("g0", "x + f32(k)"),
        call("g1", "in.uv.x + f32(k) + frame.time")
    )
}

#[test]
fn calls_before_the_guard_of_a_loop_count_once_more_than_its_rounds() {
    refused(heavy(false), &format!("runs more than {MAX_OPS} operations a pixel"));
    refused(heavy(true), &format!("runs more than {MAX_OPS} operations a pixel"));
    let once = "var<private> G: f32;
        fn g(x: f32) { G += sin(x) * cos(x) + sqrt(x); }";
    let zero = format!(
        "{once}\n{}",
        entry(
            "var i = 0u; loop { g(in.uv.x); if i < 0u { } else { break; } continuing { i = i + 1u; } } return vec4<f32>(G);"
        )
    );
    let plain = format!("{once}\n{}", entry("g(in.uv.x); return vec4<f32>(G);"));
    assert!(pack(&zero).unwrap().ops >= pack(&plain).unwrap().ops);
}

#[test]
fn every_wgsl_line_break_ends_a_comment() {
    for end in ["\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}"] {
        let chain =
            entry(&format!("// c{end}let v = in.uv.x{}; return vec4<f32>(v);", "+in.uv.x".repeat(300)));
        refused(chain, &format!("more than {MAX_OPERANDS} operations deep"));
        let nested = entry(&format!(
            "// c{end}let v = {}in.uv.x{}; return vec4<f32>(v);",
            "(".repeat(100),
            ")".repeat(100)
        ));
        refused(nested, &format!("more than {MAX_BRACKETS} brackets"));
    }
    let fine = entry("// a\u{2028}let v = in.uv.x + in.uv.y;\u{2029} return vec4<f32>(v);");
    pack(&fine).unwrap();
}
