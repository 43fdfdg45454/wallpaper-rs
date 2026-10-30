use super::*;

fn legit(name: &str) -> Descriptor {
    let path = format!("{}/tests/checks/legit/{name}.wgsl", env!("CARGO_MANIFEST_DIR"));
    let wgsl = std::fs::read_to_string(&path).unwrap();
    let json = json!({ "id": "e", "cost": {}, "passes": [{ "entry": "fs_e" }] }).to_string();
    Descriptor::packaged("e", &json, &wgsl).unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn read_only_tables_cost_like_reads_and_legit_effects_pass() {
    let names = ["bayer8", "gauss5", "kernel7x7", "palette16", "palette16_switch", "poisson16", "poisson32"];
    let names = names.into_iter().chain(["poisson64", "poisson64x2", "sdf_struct40", "sdf_struct40b"]);
    for name in names.chain(["spiral128", "unrolled33"]) {
        let units = legit(name).units();
        assert!(units < 64.0, "{name}: {units} units");
    }
    let (table, switch) = (legit("palette16").units(), legit("palette16_switch").units());
    assert!(table <= switch * 4.0 + 2.0, "palette as a table {table}, as a switch {switch}");
    assert!(
        legit("poisson64").units() <= legit("spiral128").units(),
        "taps from a table cost no more than computed ones"
    );
    assert!(legit("poisson64x2").ops < 0.05 * MAX_OPS as f32);
}

fn divided(op: &str) -> f32 {
    let steps = format!("a = a {op} b + c; b = b + 1u;").repeat(16);
    let body = format!(
        "var a = u32(in.position.x) + 7u; var b = u32(in.position.y) + 3u; let c = 5u;
         for (var i = 0u; i < 1024u; i++) {{ {steps} }} return vec4<f32>(f32(a));"
    );
    pack(&entry(&body)).unwrap().ops
}

#[test]
fn integer_division_weighs_as_much_as_it_costs() {
    let (times, plus) = (divided("*"), divided("+"));
    for op in ["/", "%"] {
        let ratio = divided(op) / times;
        assert!(ratio > 3.0 && ratio < 8.0, "{op}: {ratio} times a multiply");
    }
    assert!((times - plus).abs() < 1.0);
}
