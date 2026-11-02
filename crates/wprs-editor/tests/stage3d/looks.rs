use super::{ask, edit, project};
use serde_json::json;

#[test]
fn a_model_file_tells_its_counts_and_what_each_tier_gets() {
    let root = tempfile::tempdir().unwrap();
    let p = project(&root);
    let info = ask(&p, json!({ "model_info": { "asset": "toy" } }));
    assert_eq!(info["triangles"], json!(12 + 2 * 8 * 16));
    assert_eq!(info["materials"].as_array().unwrap().len(), 2);
    assert_eq!(info["textures"][0]["size"], json!([8, 8]));
    assert_eq!(info["tiers"].as_array().unwrap().len(), 4);
    assert!(info["tiers"].as_array().unwrap().iter().all(|t| t["fits"] == true));
    assert_eq!(info["phone"]["texture"], json!(2048));
    let file = ask(&p, json!({ "model_info": { "file": "toy.glb" } }));
    assert_eq!(file["triangles"], info["triangles"]);
    std::fs::write(p.dir.join("bad.glb"), b"nothing").unwrap();
    assert!(ask(&p, json!({ "model_info": { "file": "bad.glb" } }))["error"].is_string());
}

#[test]
fn the_materials_panel_lists_maps_with_thumbnails_and_a_preview_ball() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    edit(&mut p, json!({ "model": { "add": { "asset": "toy", "id": "doll" } } })).unwrap();
    let plain = ask(&p, json!({ "materials3d": { "layer": "doll" } }));
    edit(&mut p, json!({ "material": { "layer": "doll", "name": "paint", "color": "#00ff00" } })).unwrap();
    let list = ask(&p, json!({ "materials3d": { "layer": "doll" } }));
    let paint = &list["materials"][0];
    assert_eq!(paint["name"], "paint");
    assert!(paint["maps"][0]["thumb"].as_str().unwrap().starts_with("data:image/png;base64,"));
    assert!(paint["maps"][1]["thumb"].is_null());
    assert_eq!(paint["set"]["color"], "#00ff00");
    assert_ne!(paint["sphere"], plain["materials"][0]["sphere"]);
    assert!(list["shaders"].as_array().unwrap().contains(&json!("unlit")));
}

#[test]
fn the_cost_panel_counts_triangles_texture_memory_and_tiers_per_quality() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    edit(&mut p, json!({ "model": { "add": { "asset": "toy", "id": "doll" } } })).unwrap();
    let cost = ask(&p, json!({ "cost3d": { "quality": "q2" } }));
    assert_eq!(cost["levels"].as_array().unwrap().len(), 4);
    assert_eq!(cost["levels"][3]["triangles"], json!(268));
    assert_eq!(cost["levels"][0]["specular"], false);
    assert!(cost["tiers"][0]["memory"].as_u64().unwrap() > 0);
    let lit = ask(&p, json!({ "lighting": { "time": 0, "quality": "q1" } }));
    assert_eq!(lit["models"]["models"][0]["id"], "doll");
}
