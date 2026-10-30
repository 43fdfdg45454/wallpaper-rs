use super::*;

#[test]
fn heavy_loops_are_refused_by_their_operations() {
    let matmuls = "m = m * n;".repeat(400);
    let looped = entry(&format!(
        "let v = vec4<f32>(in.uv, 0.5, 1.0); var m = mat4x4<f32>(v, v, v, v); let n = m * 0.999;
         for (var i = 0u; i < 6u; i++) {{ for (var j = 0u; j < 1024u; j++) {{ {matmuls} }} }}
         return m[0];"
    ));
    refused(looped, &format!("runs more than {MAX_OPS} operations a pixel"));
    let scalar = entry(&format!(
        "var a = in.uv.x; for (var i = 0u; i < 12u; i++) {{ for (var j = 0u; j < 1024u; j++) {{ {} }} }} return vec4<f32>(a);",
        "a = a * a + a;".repeat(40)
    ));
    refused(scalar, "operations a pixel");
    let light = entry(
        "var c = tap(in.uv); for (var i = 0u; i < 64u; i++) { c = c * 0.99 + tap(in.uv + f32(i)); } return c;",
    );
    let d = pack(&light).unwrap();
    assert!(d.ops > 0.0 && d.ops < 10_000.0 && d.size < 2_000, "{} {}", d.ops, d.size);
}

fn fanned(body: &str, fan: &[usize]) -> String {
    let mut s = format!(
        "fn f0(m: mat4x4<f32>) -> mat4x4<f32> {{ var a = m; let n = m * 0.999; {body} return a; }}\n"
    );
    for (k, n) in fan.iter().enumerate() {
        let calls = format!("a = f{k}(a);").repeat(*n);
        s.push_str(&format!(
            "fn f{}(m: mat4x4<f32>) -> mat4x4<f32> {{ var a = m; {calls} return a; }}\n",
            k + 1
        ));
    }
    let last = fan.len();
    s + &entry(&format!("let v = vec4<f32>(in.uv, 0.5, 1.0); return f{last}(mat4x4<f32>(v, v, v, v))[0];"))
}

#[test]
fn calls_are_counted_as_often_as_drivers_inline_them() {
    let inlined = format!("is more than {MAX_INLINED} operations long once its calls are counted");
    refused(fanned(&"a = a * n;".repeat(500), &[16]), &inlined);
    refused(fanned(&"a = a * n;".repeat(200), &[16, 16, 8]), "operations a pixel");
    refused(fanned(&"a = a * n;".repeat(10), &[16, 16, 16, 16]), "operations a pixel");
    let d = pack(&fanned(&"a = a * n;".repeat(10), &[4, 4])).unwrap();
    assert!(d.size > 16 * 10 * 64 && d.size < u64::from(u32::MAX), "{}", d.size);
}

#[test]
fn stage_functions_are_held_to_the_operations_of_a_pixel() {
    for (stage, kind) in [("material", "Surface"), ("particle", "Sprite")] {
        let json = json!({ "id": "p", "stage": stage, "cost": {} }).to_string();
        let wgsl = format!(
            "fn {stage}(s: {kind}) -> {kind} {{ var o = s; let v = o.color; var m = mat4x4<f32>(v, v, v, v);
             let n = m * 0.999; for (var i = 0u; i < 1024u; i++) {{ for (var j = 0u; j < 1024u; j++) {{ {} }} }}
             o.color = m[0]; return o; }}",
            "m = m * n;".repeat(30)
        );
        let wgsl = if stage == "material" { wgsl.replace("color", "albedo") } else { wgsl };
        let fault =
            Descriptor::packaged("p", &json, &wgsl).err().unwrap_or_else(|| panic!("{stage} accepted"));
        assert!(fault.contains(&format!("{stage} runs more than {MAX_OPS} operations a pixel")), "{fault}");
        let light = wgsl.replace("1024u; j++", "2u; j++").replace("1024u; i++", "4u; i++");
        let d = Descriptor::packaged("p", &json, &light).unwrap_or_else(|e| panic!("{stage}: {e}"));
        assert!(d.ops > 4.0 * 2.0 * 30.0 * 64.0, "{stage}: {}", d.ops);
    }
}
