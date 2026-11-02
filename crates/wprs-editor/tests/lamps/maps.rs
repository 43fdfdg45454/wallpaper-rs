use super::{act, layer, project, undone};
use serde_json::json;

#[test]
fn normals_are_made_from_a_picture_and_saved_as_an_asset() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let before = p.doc.clone();
    act(&mut p, json!({ "normals": { "layer": "logo", "strength": 4, "blur": 0 } })).unwrap();
    undone(&mut p, &before);
    let id = layer(&p, "logo")["lighting"]["normal"].as_str().unwrap().to_string();
    let asset = &p.doc["scene"]["assets"][&id];
    assert_eq!(asset["linear"], true);
    let saved = image::open(p.dir.join(asset["path"].as_str().unwrap())).unwrap().into_rgba8();
    assert!(saved.get_pixel(20, 10).0[0] < 100, "the edge between dark and light tilts to the left");
    let view = wprs_editor::lamps::normals_preview(
        &p.dir,
        &p.doc,
        &serde_json::from_value(json!({ "asset": "pic" })).unwrap(),
    );
    assert!(
        view["relief"].as_str().unwrap().starts_with("data:image/png") && view["size"] == json!([40, 20])
    );
    for bad in [
        json!({ "asset": "song" }),
        json!({ "asset": "pic", "strength": 99 }),
        json!({ "layer": "nothing" }),
        json!({}),
    ] {
        assert!(act(&mut p, json!({ "normals": bad.clone() })).is_err(), "{bad}");
    }
}

#[test]
fn roughness_and_emissive_maps_are_painted_in_gray() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let before = p.doc.clone();
    let stroke =
        json!({ "layer": "logo", "map": "emissive", "radius": 0.2, "value": 1, "points": [[0.5, 0.5]] });
    act(&mut p, json!({ "paint_map": stroke })).unwrap();
    undone(&mut p, &before);
    let id = layer(&p, "logo")["lighting"]["emissive_map"].as_str().unwrap().to_string();
    let path = p.doc["scene"]["assets"][&id]["path"].as_str().unwrap().to_string();
    let map = image::open(p.dir.join(&path)).unwrap().into_rgba8();
    assert_eq!(map.dimensions(), (40, 20));
    let [r, g, b, _] = map.get_pixel(20, 10).0;
    assert!(r > 200 && r == g && g == b && map.get_pixel(0, 0).0[0] == 0);
    let rough =
        json!({ "layer": "logo", "map": "roughness", "radius": 0.2, "value": 0, "points": [[0.1, 0.5]] });
    act(&mut p, json!({ "paint_map": rough })).unwrap();
    let id = layer(&p, "logo")["lighting"]["roughness_map"].as_str().unwrap().to_string();
    let map = image::open(p.dir.join(p.doc["scene"]["assets"][&id]["path"].as_str().unwrap()))
        .unwrap()
        .into_rgba8();
    assert!(map.get_pixel(4, 10).0[0] < 60 && map.get_pixel(39, 0).0[0] == 255);
    let heat = wprs_editor::lamps::heat(&p.dir, &p.doc, "logo", "roughness_map");
    assert!(heat["heat"].as_str().unwrap().starts_with("data:image/png"));
    assert!(
        act(
            &mut p,
            json!({ "paint_map": { "layer": "logo", "map": "emissive", "radius": 0.1, "points": [] } })
        )
        .is_err()
    );
    act(&mut p, json!({ "light": { "add": { "id": "l" } } })).unwrap();
    assert!(
        act(
            &mut p,
            json!({ "paint_map": { "layer": "l", "map": "emissive", "radius": 0.1, "points": [[0, 0]] } })
        )
        .is_err()
    );
}
