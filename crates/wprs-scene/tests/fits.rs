use serde_json::json;
use wprs_scene::*;

const UNIFORM_LOOPS: [&str; 4] = ["blur_motion", "blur_radial", "depth_parallax", "god_rays"];

fn renamed(text: &str, id: &str) -> String {
    let (suffix, mut out, mut word) = (format!("_{id}"), String::new(), String::new());
    for c in text.chars().chain(['\n']) {
        if c.is_ascii_alphanumeric() || c == '_' {
            word.push(c);
            continue;
        }
        if word.len() > suffix.len() && word.ends_with(&suffix) {
            word.insert_str(word.len() - id.len(), "x_");
        }
        out.push_str(&std::mem::take(&mut word));
        out.push(c);
    }
    out.pop();
    out
}

#[test]
fn every_built_in_effect_fits_the_package_limits_with_room_to_spare() {
    let (mut measured, mut ops, mut size) = (0, 0.0f32, 0u64);
    for d in builtin() {
        let original = serde_json::to_value(d).unwrap().to_string();
        let mut json: serde_json::Value = serde_json::from_str(&renamed(&original, &d.id)).unwrap();
        let id = format!("x_{}", d.id);
        json["id"] = id.clone().into();
        json["cost"] = json.get("cost").cloned().unwrap_or(json!({}));
        let json = json.to_string();
        let checked = Descriptor::packaged(&id, &json, &renamed(&d.wgsl, &d.id));
        if UNIFORM_LOOPS.contains(&d.id.as_str()) {
            let fault = checked.err().unwrap_or_else(|| panic!("{} was accepted", d.id));
            assert!(fault.contains("a counter that a literal limits"), "{}: {fault}", d.id);
            continue;
        }
        let p = checked.unwrap_or_else(|e| panic!("{}: {e}", d.id));
        assert!(p.ops * 50.0 < MAX_OPS as f32, "{} {}", d.id, p.ops);
        assert!(p.size * 10 < MAX_INLINED, "{} {}", d.id, p.size);
        (measured, ops, size) = (measured + 1, ops.max(p.ops), size.max(p.size));
    }
    eprintln!(
        "ops {:.2}% inlined {:.2}%",
        ops * 100.0 / MAX_OPS as f32,
        size as f32 * 100.0 / MAX_INLINED as f32
    );
    assert_eq!(measured, builtin().len() - UNIFORM_LOOPS.len());
}
