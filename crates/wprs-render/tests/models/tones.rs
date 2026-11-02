use super::*;

fn bright(canvas: Value) -> Scene {
    let mut doc = json!({ "schema": SCHEMA, "canvas": { "size": [SIDE, SIDE], "background": "#000000" },
        "layers": [{ "id": "glow", "type": "solid", "color": "#ff4020", "brightness": 4 },
                   { "id": "mid", "type": "solid", "color": "#808080", "size": [16, 16] }] });
    canvas.as_object().unwrap().iter().for_each(|(k, v)| doc["canvas"][k] = v.clone());
    Scene::from_json(&doc.to_string()).unwrap()
}

#[test]
fn tone_maps_bend_bright_colors_and_none_keeps_the_old_picture() {
    let Some(r) = renderer() else { return };
    let old = render(r, bright(json!({})), Quality::Q3);
    let none = render(r, bright(json!({ "hdr": true, "tonemap": "none" })), Quality::Q3);
    assert_eq!(rgb(&old, 32, 32), rgb(&none, 32, 32));
    let aces = render(r, bright(json!({ "hdr": true, "tonemap": "aces" })), Quality::Q3);
    let agx = render(r, bright(json!({ "hdr": true, "tonemap": "agx" })), Quality::Q3);
    let (n, a, g) = (rgb(&none, 4, 4), rgb(&aces, 4, 4), rgb(&agx, 4, 4));
    assert_eq!(n[0], 255);
    assert!(a != n && g != n && a != g, "{n:?} {a:?} {g:?}");
    assert!(a[0] > a[1] && g[0] > g[1], "{a:?} {g:?}");
    let (ma, mg) = (rgb(&aces, 32, 32), rgb(&agx, 32, 32));
    assert!(ma[0] < 200 && mg[0] < 220 && ma[0] > 60 && mg[0] > 60, "{ma:?} {mg:?}");
}

#[test]
fn bright_models_keep_their_light_past_white_with_hdr() {
    let Some(r) = renderer() else { return };
    let models = BTreeMap::from([("lamp", cube(unlit("paint", [1.0, 0.25, 0.1, 1.0])))]);
    let layer = json!([{ "id": "cube", "type": "model", "model": "lamp", "brightness": 6 }]);
    let canvas = json!({ "canvas": { "size": [SIDE, SIDE], "background": "#000000", "hdr": true, "tonemap": "aces" } });
    let px = render(r, scene(canvas, layer.clone(), models.clone()), Quality::Q3);
    let flat = render(r, scene(json!({}), layer, models), Quality::Q3);
    assert!(rgb(&px, 32, 32) != rgb(&flat, 32, 32), "{:?}", rgb(&px, 32, 32));
}

#[test]
fn the_editor_split_shows_the_picture_before_the_tone_map_on_the_left() {
    let Some(r) = renderer() else { return };
    let mut s = bright(json!({ "hdr": true, "tonemap": "aces" }));
    s.ext.insert("editor".into(), json!({ "tonemap_split": 0.5 }));
    let split = render(r, s, Quality::Q3);
    let none = render(r, bright(json!({ "hdr": true })), Quality::Q3);
    let aces = render(r, bright(json!({ "hdr": true, "tonemap": "aces" })), Quality::Q3);
    assert_eq!(rgb(&split, 4, 4), rgb(&none, 4, 4));
    assert_eq!(rgb(&split, 60, 4), rgb(&aces, 60, 4));
}
