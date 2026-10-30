use serde_json::json;
use std::process::Command;
use std::time::{Duration, Instant};
use wprs_scene::Descriptor;

const PROBE: &str = "WPRS_CONSTEVAL_PROBE";

fn pack(wgsl: &str) -> Result<Descriptor, String> {
    let json = json!({ "id": "p", "cost": {}, "passes": [{ "entry": "fs_p" }] }).to_string();
    Descriptor::packaged("p", &json, wgsl)
}

fn entry(body: &str) -> String {
    format!("@fragment fn fs_p(in: VertexOut) -> @location(0) vec4<f32> {{ {body} }}")
}

fn array(item: &str, n: usize) -> String {
    format!("array({})", vec![item; n].join(","))
}

fn nested(leaf: &str, n: usize, levels: usize) -> String {
    let names = ["A", "B", "C", "D"];
    let outer = (1..levels).map(|at| format!("const {} = {};\n", names[at], array(names[at - 1], n)));
    format!("const A = {leaf};\n{}", outer.collect::<String>())
}

fn probes() -> Vec<(&'static str, String, &'static str)> {
    let one = entry("return vec4<f32>(1.0);");
    let dynamic = entry("let i = u32(in.uv.x); return vec4<f32>(f32(C[i][i][i]));");
    let (wide, wide_ints) = (nested("array<f32, 64>()", 3000, 3), nested(&array("1", 2500), 2500, 3));
    let (deep, ints) = (nested("array<f32, 64>()", 1000, 4), nested(&array("1", 1000), 1000, 3));
    let (unary, binary, large) = ("apply the unary op", "apply the binary op", "evaluation is too large");
    vec![
        ("zero array plus one", format!("const K = array<f32, 4000000000>() + 1.0;\n{one}"), binary),
        ("negated zero array", format!("const K = -array<f32, 4000000000>();\n{one}"), unary),
        ("negated zero array in fn", entry("let k = -array<f32, 4000000000>(); return vec4(k[0]);"), unary),
        ("negated wide arrays", format!("{wide}const D = -C;\n{one}"), ""),
        ("wide abstract arrays made concrete", format!("{wide_ints}{dynamic}"), ""),
        ("negated deep arrays", format!("{deep}const E = -D;\n{one}"), unary),
        ("selected deep arrays", format!("{deep}const E = select(D, D, true);\n{one}"), "for `select`"),
        ("deep arrays copied into fn", format!("{deep}{}", entry("return vec4(D[1][2][3][4]);")), large),
        ("deep abstract arrays made concrete", format!("{ints}{dynamic}"), large),
    ]
}

#[test]
#[ignore = "runs inside the memory limited child of huge_constant_values_are_refused_within_a_gibibyte"]
fn probe_constant_values() {
    if std::env::var_os(PROBE).is_none() {
        return;
    }
    let limit = libc::rlimit { rlim_cur: 1 << 30, rlim_max: 1 << 30 };
    assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_AS, &limit) }, 0);
    for (name, wgsl, problem) in probes() {
        let start = Instant::now();
        let fault = pack(&wgsl).err().unwrap_or_else(|| panic!("{name} was accepted"));
        let took = start.elapsed();
        println!("refused {name} ({} bytes) in {took:?}: {fault}", wgsl.len());
        assert!(took < Duration::from_secs(3), "{name} took {took:?}");
        assert!(fault.contains(problem), "{name}: {fault}");
    }
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let peak = status.lines().find(|l| l.starts_with("VmHWM")).unwrap_or("VmHWM unknown");
    println!("peak {peak}");
}

#[test]
fn huge_constant_values_are_refused_within_a_gibibyte() {
    let args = ["--exact", "probe_constant_values", "--ignored", "--nocapture", "--test-threads=1"];
    let me = std::env::current_exe().unwrap();
    let out = Command::new(me).args(args).env(PROBE, "1").output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    println!("{stdout}{}", String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "the probe child ended with {}", out.status);
    assert_eq!(stdout.matches("refused ").count(), probes().len());
}

#[test]
fn moderate_constant_tables_are_still_accepted() {
    let lut = (0..256).map(|i| format!("{}.0", i % 7)).collect::<Vec<_>>().join(",");
    let grid = array(&array("vec2(1, 2)", 16), 16);
    let wgsl = format!(
        "const LUT = array<f32, 256>({lut});\nconst SOFT = array({lut});\nconst GRID = {grid};\n\
         const ZERO = array<array<vec4<f32>, 8>, 8>();\nconst NEG = -vec4<f32>(1.0, 2.0, 3.0, 4.0) * 0.5;\n{}",
        entry(
            "let i = u32(in.uv.x * 255.0); let g = vec2<f32>(GRID[i % 16u][3]);
             let s = LUT[i] + LUT[7] + SOFT[i] + SOFT[9] + f32(GRID[2][5].y) + ZERO[i % 8u][1].x;
             return NEG + vec4<f32>(s, g, -LUT[3]);"
        )
    );
    pack(&wgsl).unwrap_or_else(|e| panic!("{e}"));
}
