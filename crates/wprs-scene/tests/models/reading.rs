use super::*;

#[test]
fn a_forged_cube_reads_back_with_normals_tangents_and_its_material() {
    let m = Model::decode(&cube()).unwrap();
    assert_eq!((m.triangles, m.vertex_count(), m.materials.len()), (12, 24, 1));
    assert_eq!(m.bounds, [[-1.0; 3], [1.0; 3]]);
    assert_eq!(m.materials[0].name, "paint");
    let v = &m.vertices[..MODEL_FLOATS];
    assert_eq!(&v[3..6], &[1.0, 0.0, 0.0]);
    assert!((v[8] * v[8] + v[9] * v[9] + v[10] * v[10] - 1.0).abs() < 1e-4);
    let file = ModelFile::parse(&cube()).unwrap();
    assert_eq!((file.triangles, file.materials.len()), (12, 1));
}

#[test]
fn textures_past_4096_pixels_and_broken_pictures_are_refused() {
    let mut png = vec![0x89, b'P', b'N', b'G', 13, 10, 26, 10, 0, 0, 0, 13, b'I', b'H', b'D', b'R'];
    png.extend(5000u32.to_be_bytes());
    png.extend(16u32.to_be_bytes());
    png.extend([8, 6, 0, 0, 0]);
    let mut m = ForgeMaterial::new("paint", [1.0; 4]);
    m.maps[0] = Some(png);
    let big = Forge::default().material(m.clone()).part("box", ForgeShape::Plane, [0.0; 3], 1.0, 0).glb();
    assert!(Model::decode(&big).unwrap_err().contains("4096"));
    m.maps[0] = Some(b"not a picture".to_vec());
    let odd = Forge::default().material(m).part("box", ForgeShape::Plane, [0.0; 3], 1.0, 0).glb();
    assert!(Model::decode(&odd).unwrap_err().contains("PNG, JPEG or WebP"));
    assert_eq!(picture_size(&[0xFF, 0xD8, 0xFF, 0xC0, 0, 17, 8, 0, 40, 0, 30, 3]), Some([30, 40]));
}

#[test]
fn each_tier_caps_triangles_bones_textures_and_shadowed_lights() {
    let caps: Vec<ModelCaps> =
        [Tier::T0, Tier::T1, Tier::T2, Tier::T3].map(|t| model_caps(t, false)).to_vec();
    assert_eq!(caps.iter().map(|c| c.triangles).collect::<Vec<_>>(), [50_000, 150_000, 500_000, 2_000_000]);
    assert_eq!(caps.iter().map(|c| c.shadows).collect::<Vec<_>>(), [0, 0, 1, 2]);
    assert_eq!((caps[1].texture, caps[3].texture, model_caps(Tier::T3, true).texture), (2048, 4096, 2048));
    assert_eq!(caps[0].bones, 256);
    let ball = Forge::default()
        .material(ForgeMaterial::new("m", [1.0; 4]))
        .part("b", ForgeShape::Sphere(130), [0.0; 3], 1.0, 0)
        .glb();
    let ball = Model::decode(&ball).unwrap();
    assert!(ball.triangles > 50_000 && ball.triangles < 150_000);
    let low = model_verdict(&[&ball], 0, Tier::T0);
    assert!(!low.fits && low.reasons[0].contains("50000"), "{low:?}");
    assert!(model_verdict(&[&ball], 0, Tier::T1).fits);
    assert!(!model_verdict(&[&ball], 300, Tier::T3).fits);
}

#[test]
fn models_and_cameras_name_their_features() {
    let plain = scene(json!({ "camera3d": { "fov": 50 } }), json!([model("m", json!({}))]));
    let f = plain.features();
    assert!(f.contains("model3d") && f.contains("camera3d") && !f.contains("model_animation"), "{f:?}");
    let posed = json!({ "schema": SCHEMA, "canvas": { "size": [200, 100] }, "assets": { "toy": { "path": "toy.glb" } },
        "layers": [model("m", json!({ "clips": [{ "id": "walk" }] }))] });
    assert!(Scene::from_json(&posed.to_string()).unwrap().features().contains("model_animation"));
    let flown = json!({ "camera3d": { "paths": [{ "id": "a", "keys": [{ "t": 0, "eye": [0, 0, 5], "center": 0 }, { "t": 1, "eye": [1, 0, 5], "center": 0 }] }] } });
    assert!(scene(flown, json!([])).features().contains("camera_paths"));
    let toned = scene(json!({ "canvas": { "size": [200, 100], "hdr": true, "tonemap": "agx" } }), json!([]));
    assert!(toned.features().contains("hdr"));
    let costly = scene(json!({}), json!([model("m", json!({}))]));
    assert!(costly.cost(Quality::Q2) > scene(json!({}), json!([])).cost(Quality::Q2));
}

#[test]
fn editor_marks_reach_the_renderer_only_through_a_preview() {
    let mut s =
        scene(json!({ "ext": { "editor": { "debug3d": "shadows", "tonemap_split": 2 } } }), json!([]));
    assert_eq!(s.draft, Drafted::default(), "a loaded package keeps the editor marks out of the picture");
    s.drafted();
    assert_eq!(s.draft, Drafted { debug3d: 3, split: 1.0 });
    s.ext.clear();
    s.drafted();
    assert_eq!(s.draft, Drafted::default());
}
