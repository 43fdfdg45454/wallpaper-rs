use super::{act, layer, project, undone};
use image::{Rgba, RgbaImage};
use serde_json::{Value, json};

fn asset<'a>(p: &'a wprs_editor::Project, id: &str) -> &'a Value {
    &p.doc["scene"]["assets"][id]
}

fn bump(p: &mut wprs_editor::Project) {
    RgbaImage::from_pixel(8, 8, Rgba([128, 128, 255, 255])).save(p.dir.join("bump.png")).unwrap();
    act(p, json!({ "set": { "pointer": "/scene/assets/bump", "value": { "path": "bump.png" } } })).unwrap();
}

#[test]
fn a_map_picked_only_for_light_is_read_as_linear_in_the_same_step() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    bump(&mut p);
    let before = p.doc.clone();
    act(&mut p, json!({ "lighting": { "layer": "logo", "normal": "bump", "emissive_map": "bump" } }))
        .unwrap();
    undone(&mut p, &before);
    assert_eq!(asset(&p, "bump"), &json!({ "path": "bump.png", "linear": true }));
    assert_eq!(layer(&p, "logo")["lighting"]["emissive_map"], "bump");
    act(&mut p, json!({ "lighting": { "layer": "logo", "env": "pic" } })).unwrap();
    assert!(asset(&p, "pic").get("linear").is_none(), "a reflected picture keeps its colors");
    act(&mut p, json!({ "particles": { "lit": { "system": "flakes", "lit": true, "normal": "bump" } } }))
        .unwrap();
    assert_eq!(p.doc["scene"]["particles"]["flakes"]["render"]["normal"], "bump");
}

#[test]
fn a_picture_shown_in_colors_gets_a_linear_twin_for_particles() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let before = p.doc.clone();
    act(
        &mut p,
        json!({ "particles": { "refract": { "system": "flakes", "normal": "pic", "amount": 0.4 } } }),
    )
    .unwrap();
    undone(&mut p, &before);
    let render = &p.doc["scene"]["particles"]["flakes"]["render"];
    assert_eq!(render["refract"]["normal"], "pic_linear");
    assert_eq!(asset(&p, "pic_linear")["linear"], true);
    assert!(asset(&p, "pic").get("linear").is_none());
}

#[test]
fn painting_a_picked_map_keeps_the_picture_and_paints_a_copy() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    act(&mut p, json!({ "set": { "pointer": "/scene/assets/pic/mipmaps", "value": true } })).unwrap();
    act(&mut p, json!({ "lighting": { "layer": "logo", "roughness_map": "pic" } })).unwrap();
    let picked = layer(&p, "logo")["lighting"]["roughness_map"].as_str().unwrap().to_string();
    let twin = asset(&p, &picked).clone();
    let stroke =
        json!({ "layer": "logo", "map": "roughness", "radius": 0.2, "value": 0.1, "points": [[0.5, 0.5]] });
    let before = p.doc.clone();
    act(&mut p, json!({ "paint_map": stroke.clone() })).unwrap();
    undone(&mut p, &before);
    assert_eq!(
        asset(&p, "pic"),
        &json!({ "path": "pic.png", "mipmaps": true }),
        "the layer image is untouched"
    );
    assert_eq!(asset(&p, &picked), &twin, "the picked map is untouched");
    let painted = layer(&p, "logo")["lighting"]["roughness_map"].as_str().unwrap().to_string();
    assert_ne!(painted, picked);
    assert!(asset(&p, &painted)["path"].as_str().unwrap().starts_with("maps/"));
    act(&mut p, json!({ "paint_map": stroke })).unwrap();
    assert_eq!(
        layer(&p, "logo")["lighting"]["roughness_map"],
        painted.as_str(),
        "its own map is painted again"
    );
}

#[test]
fn a_map_two_layers_share_is_copied_before_it_is_painted() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let stroke =
        json!({ "layer": "logo", "map": "emissive", "radius": 0.2, "value": 1, "points": [[0.5, 0.5]] });
    act(&mut p, json!({ "paint_map": stroke.clone() })).unwrap();
    let first = layer(&p, "logo")["lighting"]["emissive_map"].as_str().unwrap().to_string();
    let other =
        json!({ "id": "other", "type": "image", "image": "pic", "lighting": { "emissive_map": first } });
    act(&mut p, json!({ "add": { "pointer": "/scene/layers/-", "value": other } })).unwrap();
    let path = asset(&p, &first)["path"].clone();
    act(&mut p, json!({ "paint_map": stroke })).unwrap();
    assert_eq!(asset(&p, &first)["path"], path, "the other layer keeps its map");
    assert_ne!(layer(&p, "logo")["lighting"]["emissive_map"], first.as_str());
}

#[test]
fn normals_refuse_an_id_that_names_another_asset() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let made = |id: &str| json!({ "normals": { "layer": "logo", "strength": 2, "blur": 0, "id": id } });
    assert!(act(&mut p, made("pic")).is_err());
    assert_eq!(asset(&p, "pic"), &json!({ "path": "pic.png" }));
    act(&mut p, made("logo_bumps")).unwrap();
    act(&mut p, made("logo_bumps")).unwrap();
    assert!(asset(&p, "logo_bumps")["path"].as_str().unwrap().starts_with("normals/"));
}

#[test]
fn a_particles_layer_takes_strength_and_glow() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let before = p.doc.clone();
    act(&mut p, json!({ "lighting": { "layer": "flakes", "strength": 0.4, "emissive": 3 } })).unwrap();
    undone(&mut p, &before);
    assert_eq!(layer(&p, "flakes")["lighting"], json!({ "strength": 0.4, "emissive": 3.0 }));
    for bad in [json!({ "normal": "pic" }), json!({ "roughness": 0.3 }), json!({ "emissive": 20 })] {
        let mut args = bad.clone();
        args["layer"] = json!("flakes");
        assert!(act(&mut p, json!({ "lighting": args })).is_err(), "{bad}");
    }
    act(&mut p, json!({ "lighting": { "layer": "flakes", "off": true } })).unwrap();
    assert!(layer(&p, "flakes").get("lighting").is_none());
}
