use super::*;

fn ball(material: ForgeMaterial) -> BTreeMap<&'static str, Vec<u8>> {
    BTreeMap::from([(
        "ball",
        Forge::default().material(material).part("ball", ForgeShape::Sphere(24), [0.0; 3], 3.0, 0).glb(),
    )])
}

fn lit_scene(material: ForgeMaterial, extra: Value) -> Scene {
    let mut layer = json!({ "id": "ball", "type": "model", "model": "ball" });
    extra.as_object().unwrap().iter().for_each(|(k, v)| layer[k] = v.clone());
    scene(json!({}), json!([layer]), ball(material))
}

#[test]
fn a_pbr_ball_is_brighter_where_it_faces_the_light() {
    let Some(r) = renderer() else { return };
    let px = render(r, lit_scene(ForgeMaterial::new("paint", [0.9, 0.9, 0.9, 1.0]), json!({})), Quality::Q3);
    let (lit, dark) = (rgb(&px, 22, 22), rgb(&px, 44, 44));
    assert!(lit[0] > dark[0] + 20, "{lit:?} {dark:?}");
    assert_eq!(rgb(&px, 1, 1), [0, 0, 0]);
}

#[test]
fn every_map_of_a_material_reaches_the_picture() {
    let Some(r) = renderer() else { return };
    let mut m = ForgeMaterial::new("paint", [1.0; 4]);
    m.maps = [
        Some(png([0, 200, 0, 255], 8)),
        Some(png([255, 255, 0, 255], 8)),
        Some(png([128, 128, 255, 255], 8)),
        Some(png([255, 255, 255, 255], 8)),
        Some(png([255, 0, 0, 255], 8)),
    ];
    m.emissive = [1.0, 0.0, 0.0];
    let px = render(r, lit_scene(m, json!({})), Quality::Q3);
    let c = rgb(&px, 32, 32);
    assert!(c[1] > 60 && c[0] > 150 && c[2] < 60, "{c:?}");
}

#[test]
fn materials_of_the_scene_override_the_file() {
    let Some(r) = renderer() else { return };
    let over = json!({ "materials": { "paint": { "shader": "unlit", "color": "#0000ff" } } });
    let px = render(r, lit_scene(ForgeMaterial::new("paint", [1.0; 4]), over), Quality::Q3);
    assert!(near(rgb(&px, 32, 32), [0, 0, 255], 3), "{:?}", rgb(&px, 32, 32));
    let glow =
        json!({ "materials": { "paint": { "lit": false, "emissive": "#ff0000", "color": "#000000" } } });
    let px = render(r, lit_scene(ForgeMaterial::new("paint", [1.0; 4]), glow), Quality::Q3);
    assert!(near(rgb(&px, 32, 32), [255, 0, 0], 3), "{:?}", rgb(&px, 32, 32));
}

#[test]
fn a_double_sided_plane_shows_its_back() {
    let Some(r) = renderer() else { return };
    let card = |two: bool| {
        let mut m = unlit("paint", [0.0, 1.0, 0.0, 1.0]);
        m.double_sided = two;
        let glb = Forge::default().material(m).part("card", ForgeShape::Plane, [0.0; 3], 3.0, 0).glb();
        let layer = json!([{ "id": "card", "type": "model", "model": "card", "rotation": [0, 180, 0] }]);
        render(r, scene(json!({}), layer, BTreeMap::from([("card", glb)])), Quality::Q3)
    };
    assert_eq!(rgb(&card(false), 32, 32), [0, 0, 0]);
    assert!(near(rgb(&card(true), 32, 32), [0, 255, 0], 3));
}

#[test]
fn a_masked_material_cuts_out_and_a_blended_one_lets_the_back_through() {
    let Some(r) = renderer() else { return };
    let mut m = unlit("paint", [1.0, 0.0, 0.0, 0.3]);
    m.alpha = Some("MASK");
    let px = render(r, lit_scene(m.clone(), json!({})), Quality::Q3);
    assert_eq!(rgb(&px, 32, 32), [0, 0, 0]);
    m.alpha = Some("BLEND");
    let px = render(r, lit_scene(m, json!({})), Quality::Q3);
    let c = rgb(&px, 32, 32);
    assert!(c[0] > 100 && c[0] < 200 && c[1] == 0, "{c:?}");
}

#[test]
fn an_env_picture_tints_a_shiny_ball_and_the_lowest_quality_drops_its_reflection() {
    let Some(r) = renderer() else { return };
    let mut m = ForgeMaterial::new("paint", [1.0; 4]);
    (m.metallic, m.roughness) = (1.0, 0.1);
    let layer = json!([{ "id": "ball", "type": "model", "model": "ball", "env": "sky" }]);
    let s = scene(json!({ "assets": { "sky": { "path": "sky.png" } } }), layer, ball(m));
    let sky = Pixels::new(RgbaImage::from_pixel(8, 8, Rgba([0, 120, 255, 255])));
    let high = shoot(r, s.clone(), &[("sky", sky.clone())]).frame(r, 0.0, Quality::Q3);
    let low = shoot(r, s, &[("sky", sky)]).frame(r, 0.0, Quality::Q0);
    let (h, l) = (rgb(&high, 32, 32), rgb(&low, 32, 32));
    assert!(h[2] > h[0] + 40, "{h:?}");
    assert!(h[2] > l[2] + 20, "{h:?} {l:?}");
}
