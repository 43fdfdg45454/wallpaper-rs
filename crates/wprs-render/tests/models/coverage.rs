use super::*;

fn placed(id: &str, asset: &str, at: [f32; 3], extra: Value) -> Value {
    let mut l = json!({ "id": id, "type": "model", "model": asset, "position": at });
    extra.as_object().unwrap().iter().for_each(|(k, v)| l[k] = v.clone());
    l
}

fn world(layers: Value) -> Value {
    json!([{ "id": "world", "type": "group", "space": "3d", "layers": layers }])
}

fn card(m: ForgeMaterial) -> Vec<u8> {
    Forge::default().material(m).part("card", ForgeShape::Plane, [0.0; 3], 3.0, 0).glb()
}

#[test]
fn opaque_materials_ignore_the_alpha_of_their_texture_and_factor() {
    let Some(r) = renderer() else { return };
    let mut clear = unlit("paint", [1.0, 0.0, 0.0, 0.3]);
    clear.maps[0] = Some(png([255, 255, 255, 0], 4));
    let models = BTreeMap::from([("red", cube(clear)), ("blue", cube(unlit("paint", [0.0, 0.0, 1.0, 1.0])))]);
    let both = world(json!([
        placed("front", "red", [0.0, 0.0, 1.5], json!({})),
        placed("back", "blue", [0.0, 0.0, -2.0], json!({}))
    ]));
    let px = render(r, scene(json!({}), both, models.clone()), Quality::Q3);
    assert!(near(rgb(&px, 32, 32), [255, 0, 0], 3), "{:?}", rgb(&px, 32, 32));
    let lone =
        render(r, scene(json!({}), json!([placed("m", "red", [0.0; 3], json!({}))]), models), Quality::Q3);
    assert!(near(rgb(&lone, 32, 32), [255, 0, 0], 3), "{:?}", rgb(&lone, 32, 32));
}

#[test]
fn a_keyed_out_card_never_hides_the_models_behind_it() {
    let Some(r) = renderer() else { return };
    let models = BTreeMap::from([
        ("card", card(unlit("skin", [0.0, 1.0, 0.0, 1.0]))),
        ("red", cube(unlit("paint", [1.0, 0.0, 0.0, 1.0]))),
    ]);
    let key = json!({ "skin": { "shader": "chroma", "params": { "key": "#00ff00", "threshold": 0.2, "softness": 0.1 } } });
    let both = world(json!([
        placed("card", "card", [0.0, 0.0, 2.0], json!({ "materials": key })),
        placed("cube", "red", [0.0, 0.0, -1.0], json!({}))
    ]));
    let px = render(r, scene(json!({}), both, models), Quality::Q3);
    assert!(near(rgb(&px, 32, 32), [255, 0, 0], 3), "{:?}", rgb(&px, 32, 32));
}

#[test]
fn a_lone_model_at_half_opacity_is_drawn_at_half_once() {
    let Some(r) = renderer() else { return };
    let models = BTreeMap::from([("red", cube(unlit("paint", [1.0, 0.0, 0.0, 1.0])))]);
    let half = json!([placed("m", "red", [0.0; 3], json!({ "opacity": 0.5 }))]);
    let flat = json!([{ "id": "s", "type": "solid", "color": "#ff0000", "size": [16, 16], "opacity": 0.5 }]);
    let want = rgb(&render(r, scene(json!({}), flat, models.clone()), Quality::Q3), 32, 32);
    assert!(want[0] > 100 && want[0] < 200, "{want:?}");
    let px = render(r, scene(json!({}), half, models.clone()), Quality::Q3);
    assert!(near(rgb(&px, 32, 32), want, 4), "{:?} for {want:?}", rgb(&px, 32, 32));
    let grouped = world(json!([placed("m", "red", [0.0; 3], json!({ "opacity": 0.5 }))]));
    let px = render(r, scene(json!({}), grouped, models), Quality::Q3);
    assert!(near(rgb(&px, 32, 32), want, 4), "in a 3D group: {:?}", rgb(&px, 32, 32));
}
