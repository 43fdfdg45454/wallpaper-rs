use naga::{Block, Statement};
use serde_json::json;
use wprs_scene::*;

const ENDLESS: &str = "override neg: i32 = -1;
fn spin(c: vec4<f32>) -> vec4<f32> {
    var d = c;
    loop { d *= 0.5; if d.a < -1.0 { break; } }
    return d;
}
@fragment
fn fs_g(in: VertexOut) -> @location(0) vec4<f32> {
    var c = spin(tap(in.uv));
    var k: u32 = bitcast<u32>(neg);
    for (;;) {
        for (var i = 0u; i <= u32(neg); i++) { c *= 0.9; }
        while c.a > -1.0 { c *= 0.9; }
        if c.a < -1.0 { break; }
    }
    return c * f32(k & 1u);
}";

fn loops(b: &Block, out: &mut Vec<Block>) {
    for s in b.iter() {
        match s {
            Statement::Block(b) => loops(b, out),
            Statement::If { accept, reject, .. } => [accept, reject].into_iter().for_each(|b| loops(b, out)),
            Statement::Switch { cases, .. } => cases.iter().for_each(|c| loops(&c.body, out)),
            Statement::Loop { body, continuing, .. } => {
                out.push(body.clone());
                loops(body, out);
                loops(continuing, out);
            }
            _ => {}
        }
    }
}

fn counted(body: &Block) -> bool {
    let stop = |s: &Statement| matches!(s, Statement::If { accept, .. } if matches!(accept.iter().as_slice(), [Statement::Break]));
    match body.iter().as_slice() {
        [
            Statement::Emit(_),
            Statement::Store { .. },
            a,
            Statement::Emit(_),
            Statement::Store { .. },
            b,
            ..,
        ] => stop(a) && stop(b),
        _ => false,
    }
}

#[test]
fn every_loop_of_a_package_shader_gets_a_counter_even_ones_the_check_never_saw() {
    let plain = "@fragment fn fs_g(in: VertexOut) -> @location(0) vec4<f32> { return tap(in.uv); }";
    let json = json!({ "id": "g", "cost": {}, "passes": [{ "entry": "fs_g" }] }).to_string();
    let checked = Descriptor::packaged("g", &json, plain).unwrap();
    assert!(
        Descriptor::packaged("g", &json, ENDLESS).unwrap_err().contains("a counter that a literal limits")
    );
    let module = guarded(&Descriptor { wgsl: ENDLESS.into(), ..checked }).unwrap();
    let mut found = Vec::new();
    module.entry_points.iter().for_each(|e| loops(&e.function.body, &mut found));
    module.functions.iter().for_each(|(_, f)| loops(&f.body, &mut found));
    assert!(found.len() >= 4, "{}", found.len());
    assert!(found.iter().all(counted));
    let entry = module.entry_points.iter().find(|e| e.name == "fs_g").unwrap();
    assert!(
        entry
            .function
            .local_variables
            .iter()
            .all(|(_, l)| l.init.is_none_or(|h| entry.function.expressions[h].needs_pre_emit()))
    );
    assert!(matches!(entry.function.body.iter().next(), Some(Statement::Store { .. })));
}
