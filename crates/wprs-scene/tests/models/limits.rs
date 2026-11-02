use super::*;

fn issues(layers: Value) -> Vec<String> {
    let doc = json!({ "schema": SCHEMA, "canvas": { "size": [8, 8] }, "assets": { "toy": { "path": "toy.glb" } },
        "layers": layers });
    Scene::from_json(&doc.to_string())
        .unwrap()
        .validate(&BTreeMap::new())
        .iter()
        .map(|i| i.to_string())
        .collect()
}

fn light(extra: Value) -> Value {
    let mut l = json!({ "id": "sun", "type": "light", "light": "directional" });
    extra.as_object().unwrap().iter().for_each(|(k, v)| l[k] = v.clone());
    l
}

fn shaded(material: Value) -> Value {
    json!([model("m", json!({ "materials": { "paint": material } }))])
}

#[test]
fn shadow_fields_are_checked_and_need_shadows() {
    assert!(
        issues(json!([light(json!({ "shadows": true, "shadow_resolution": 1024, "shadow_bias": 0.3 }))]))
            .is_empty()
    );
    for bad in [
        json!({ "shadows": true, "shadow_resolution": 100 }),
        json!({ "shadows": true, "shadow_resolution": 8192 }),
        json!({ "shadows": true, "shadow_bias": 2 }),
        json!({ "shadow_resolution": 1024 }),
        json!({ "shadow_bias": 0.1 }),
    ] {
        assert!(!issues(json!([light(bad.clone())])).is_empty(), "{bad}");
    }
    let three: Vec<Value> = (0..3)
        .map(|i| json!({ "id": format!("l{i}"), "type": "light", "light": "spot", "shadows": true }))
        .collect();
    assert!(!issues(json!(three)).is_empty(), "at most two shadowed lights");
}

#[test]
fn built_in_material_shaders_check_their_params() {
    assert!(issues(shaded(json!({ "shader": "fur", "params": { "layers": 16, "comb_x": 0.5 } }))).is_empty());
    for bad in [
        json!({ "shader": "fur", "params": { "layers": 40 } }),
        json!({ "shader": "fur", "params": { "comb_y": 3 } }),
        json!({ "shader": "foliage", "params": { "strength": 20 } }),
        json!({ "shader": "chroma", "params": { "softness": 2 } }),
        json!({ "shader": "fur", "params": { "speed": 1 } }),
        json!({ "shader": "nothing_here" }),
    ] {
        assert!(!issues(shaded(bad.clone())).is_empty(), "{bad}");
    }
}

#[test]
fn tiers_cap_shadow_maps_and_fur_shells() {
    use wprs_scene::Beam::*;
    let side = |b, set, tier, phone, q| shadow_side(b, set, (tier, phone), q);
    assert_eq!(side(Directional, None, Tier::T3, false, Quality::Q3), 2048);
    assert_eq!(side(Spot, None, Tier::T3, false, Quality::Q3), 1024);
    assert_eq!(side(Point, None, Tier::T3, false, Quality::Q3), 512);
    assert_eq!(side(Directional, Some(4096), Tier::T3, false, Quality::Q3), 4096);
    assert_eq!(side(Directional, Some(4096), Tier::T2, false, Quality::Q3), 2048);
    assert_eq!(side(Directional, None, Tier::T3, true, Quality::Q3), 1024);
    assert_eq!(side(Directional, None, Tier::T1, false, Quality::Q3), 0);
    assert_eq!(side(Spot, None, Tier::T3, false, Quality::Q0), 512);
    assert_eq!(
        [0, 1, 2, 3].map(|t| model_caps([Tier::T0, Tier::T1, Tier::T2, Tier::T3][t], false).shadows),
        [0, 0, 1, 2]
    );
    let shells = [Quality::Q0, Quality::Q1, Quality::Q2, Quality::Q3].map(|q| fur_shells(12, q));
    assert_eq!(shells, [3, 6, 9, 12]);
    assert_eq!(fur_shells(1, Quality::Q0), 1);
}

#[test]
fn posed_models_refuse_broken_skins() {
    let glb = ForgeRig::new(2.0, 0.5).glb();
    assert!(Model::decode_posed(&glb).is_ok());
    let needle = b"\"joints\":[0,1]";
    let at = glb.windows(needle.len()).position(|w| w == needle).unwrap();
    let mut broken = glb.clone();
    broken[at + needle.len() - 2] = b'9';
    assert!(Model::decode_posed(&broken).unwrap_err().contains("missing node"));
}

#[test]
fn the_rigged_fixture_is_the_forged_column() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/rig.glb");
    let made = ForgeRig::new(2.0, 0.5).glb();
    if std::env::var_os("WPRS_WRITE_FIXTURES").is_some() {
        std::fs::write(path, &made).unwrap();
    }
    assert_eq!(std::fs::read(path).unwrap(), made);
}
