use super::*;

#[test]
fn remap_curves_and_cutout_ramps_plot_what_the_engine_computes() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let remap = json!({ "input": "life", "in": [0, 10], "clamp_in": true, "output": "size", "out": [2, 8] });
    edit(&mut p, "remap", json!({ "remap": remap })).unwrap();
    edit(&mut p, "cutout", json!({ "range": [0.25, 0.75, 0.5] })).unwrap();
    let h = handles(&p);
    let curve = &h["remaps"][0]["curve"];
    let points = curve["points"].as_array().unwrap();
    let at = |i: usize| [points[i][0].as_f64().unwrap(), points[i][1].as_f64().unwrap()];
    assert_eq!((at(0), at(points.len() - 1)), ([-2.5, 2.0], [12.5, 8.0]));
    let middle = points.iter().find(|p| (p[0].as_f64().unwrap() - 5.0).abs() < 0.2).unwrap();
    assert!((middle[1].as_f64().unwrap() - 5.0).abs() < 0.2, "{middle}");
    let ramp = h["cutout"]["ramp"]["points"].as_array().unwrap();
    assert_eq!((ramp[0][1].as_f64(), ramp[ramp.len() - 1][1].as_f64()), (Some(0.0), Some(0.5)));
    assert!(h["inputs"].as_array().unwrap().contains(&json!("distance:p0")));
    assert_eq!(h["orient"]["mode"], "screen");
}

#[test]
fn particle_shaders_offer_package_effects_of_the_particle_stage() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    std::fs::create_dir_all(p.dir.join("effects")).unwrap();
    let wgsl =
        "fn particle(p: Sprite) -> Sprite {\n    var o = p;\n    o.size = p.size * 2.0;\n    return o;\n}\n";
    std::fs::write(p.dir.join("effects/grow.json"), r#"{ "id": "grow", "stage": "particle", "cost": {} }"#)
        .unwrap();
    std::fs::write(p.dir.join("effects/grow.wgsl"), wgsl).unwrap();
    let files = json!({ "descriptor": "effects/grow.json", "wgsl": "effects/grow.wgsl" });
    act(&mut p, json!({ "set": { "pointer": "/scene/effects/grow", "value": files } })).unwrap();
    let offered = handles(&p)["shaders"]["offered"].clone();
    assert_eq!(offered, json!(["grow"]));
    edit(&mut p, "shader", json!({ "shader": "grow" })).unwrap();
    let shown = handles(&p)["shaders"].clone();
    assert_eq!((shown["current"].clone(), shown["warning"].clone()), (json!("grow"), json!(true)));
    edit(&mut p, "shader", json!({})).unwrap();
    assert!(system(&p)["render"].get("shader").is_none());
}
