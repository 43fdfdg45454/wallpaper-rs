use super::tree;

fn checked(body: &str) -> Result<(), String> {
    let source = format!("fn f(x: f32) -> f32 {{ {body} }}");
    tree(&naga::front::wgsl::parse_str(&source).map_err(|e| e.to_string())?)
}

#[test]
fn deep_expressions_and_nested_blocks_are_refused_on_the_parsed_shader() {
    let deep = checked(&format!("return x{};", "+x".repeat(300))).unwrap_err();
    assert!(deep.contains("256 operations deep"), "{deep}");
    let signs = checked(&format!("return {}x;", "- ".repeat(260))).unwrap_err();
    assert!(signs.contains("operations deep"), "{signs}");
    checked(&format!("let a = x{}; return a{};", "+x".repeat(200), "+x".repeat(200))).unwrap();
    let blocks = checked(&format!("var a = x; {} a = 1.0; {} return a;", "{".repeat(70), "}".repeat(70)));
    assert!(blocks.unwrap_err().contains("64 blocks"));
    let list = vec!["x"; 1000].join(",");
    let lets: String = "abcde".chars().map(|n| format!("let {n} = array<f32, 1000>({list});")).collect();
    let many = checked(&format!("{lets} return a[0] + b[0] + c[0] + d[0] + e[0];")).unwrap_err();
    assert!(many.contains("4096 arguments"), "{many}");
}
