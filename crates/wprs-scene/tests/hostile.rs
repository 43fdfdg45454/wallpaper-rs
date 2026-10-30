use serde_json::json;
use wprs_scene::*;

fn pack(wgsl: &str) -> Result<Descriptor, String> {
    let json = json!({ "id": "p", "cost": {}, "passes": [{ "entry": "fs_p" }] }).to_string();
    Descriptor::packaged("p", &json, wgsl)
}

fn small_stack(wgsl: String) -> Result<Descriptor, String> {
    let checked = std::thread::Builder::new().stack_size(2 << 20).spawn(move || pack(&wgsl)).unwrap();
    checked.join().expect("the check returns instead of crashing")
}

fn entry(body: &str) -> String {
    format!("@fragment fn fs_p(in: VertexOut) -> @location(0) vec4<f32> {{ {body} }}")
}

fn refused(wgsl: String, problem: &str) {
    let size = wgsl.len();
    let fault = small_stack(wgsl).err().unwrap_or_else(|| panic!("a {size} byte shader was accepted"));
    assert!(fault.contains(problem), "{size} bytes: {fault}");
}

#[test]
fn long_chains_and_deep_nesting_are_refused_before_naga_reads_them() {
    let n = 5000;
    let chains = [
        entry(&format!("let a = in.uv.x; let b = a{}; return vec4<f32>(b);", "+a".repeat(n))),
        entry(&format!("let a = in.uv.x{}; return vec4<f32>(a);", "*in.uv.y".repeat(1000))),
        entry(&format!("let a = {}in.uv.x; return vec4<f32>(a);", "- ".repeat(n))),
        format!("const K = 1.0{};\n{}", "+1.0".repeat(n), entry("return vec4<f32>(K);")),
        entry(&format!("let v = in.position{}; return v;", ".xyzw".repeat(n))),
        entry(&format!("let a = 1.0{}; return vec4<f32>(a);", ".x".repeat(30000))),
        entry(&format!("let a = {}0.0; return vec4<f32>(a);", "0.e+".repeat(7500))),
        entry(&format!("let a = in.uv{}; return vec4<f32>(a);", "[0]".repeat(20000))),
        entry(&format!(
            "var a = in.uv.x; if a > 0.0 {{ a = 1.0; }}{} return vec4<f32>(a);",
            " else if a > 0.5 { a = 2.0; }".repeat(300)
        )),
    ];
    chains.into_iter().for_each(|w| refused(w, &format!("more than {MAX_OPERANDS} operations deep")));
    let nested = [
        entry(&format!("let a = {}in.uv.x{}; return vec4<f32>(a);", "(".repeat(n), ")".repeat(n))),
        entry(&format!("let a = {}in.uv.x{}; return vec4<f32>(a);", "sin(".repeat(1000), ")".repeat(1000))),
        entry(&format!("var a = in.uv.x; {} a = 1.0; {} return vec4<f32>(a);", "{".repeat(n), "}".repeat(n))),
        entry(&format!(
            "var a = 0.0; {} a = 1.0; {} return vec4<f32>(a);",
            "if a > 0.5 {".repeat(300),
            "}".repeat(300)
        )),
        entry(&format!(
            "let a = {}in.uv.x{}; return vec4<f32>(a);",
            "array<f32,1>(".repeat(500),
            ")[0]".repeat(500)
        )),
    ];
    nested.into_iter().for_each(|w| refused(w, &format!("more than {MAX_BRACKETS} brackets")));
    let fine = [
        entry(&format!("let a = in.uv.x; let b = a{}; return vec4<f32>(b);", "+a".repeat(200))),
        entry(&format!("let a = {}in.uv.x{}; return vec4<f32>(a);", "sin(".repeat(40), ")".repeat(40))),
        entry(&format!(
            "var a = /* {} */ in.uv.x; return vec4<f32>(a * 1e-3 + 0x1p-3 + .5e+2);",
            "+(".repeat(n)
        )),
    ];
    fine.into_iter().for_each(|w| drop(small_stack(w).unwrap_or_else(|e| panic!("{e}"))));
    let listed = |n: usize, item: &str| vec![item; n].join(", ");
    let constants = format!(
        "const A = array({});\nconst B = array({});\nconst C = array({});\n{}",
        listed(2500, "1"),
        listed(2500, "A"),
        listed(2500, "B"),
        entry("let i = u32(in.position.x) % 4u; return vec4<f32>(f32(C[i][i][i]));")
    );
    refused(constants, &format!("more than {MAX_ARGUMENTS} arguments to calls and constructors"));
}
