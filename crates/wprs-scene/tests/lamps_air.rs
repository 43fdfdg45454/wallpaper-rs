use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_scene::*;

fn scene(layers: Json) -> Scene {
    let assets = json!({ "wall": { "path": "wall.png" }, "song": { "path": "song.ogg" }, "clip": { "path": "clip.mp4" } });
    let doc = json!({ "schema": SCHEMA, "canvas": { "size": [200, 100] }, "assets": assets, "layers": layers,
        "particles": { "rain": { "capacity": 10, "emitters": [{ "shape": "edge" }] } } });
    Scene::from_json(&doc.to_string()).unwrap()
}

fn issues(layers: Json) -> Vec<String> {
    scene(layers).validate(&BTreeMap::new()).iter().map(ToString::to_string).collect()
}

fn spot(id: &str, extra: Json) -> Json {
    let mut l = json!({ "id": id, "type": "light", "light": "spot", "transform": { "position": [50, 50] } });
    l.as_object_mut().unwrap().extend(extra.as_object().cloned().unwrap_or_default());
    l
}

fn wall(id: &str, lit: bool) -> Json {
    let mut w = json!({ "id": id, "type": "image", "image": "wall" });
    if lit {
        w["lighting"] = json!({});
    }
    w
}

#[test]
fn volumetrics_keep_density_and_samples_in_range_and_only_on_points_and_spots() {
    let bad = [
        json!({ "density": 2 }),
        json!({ "samples": 2 }),
        json!({ "samples": 65 }),
        json!({ "density": -1 }),
        json!({ "shafts": 1.5 }),
        json!({ "shafts": -0.1 }),
    ];
    for v in bad {
        assert!(!issues(json!([spot("s", json!({ "volumetric": v }))])).is_empty(), "{v}");
    }
    let tube = json!({ "id": "t", "type": "light", "light": "tube", "volumetric": {} });
    assert!(issues(json!([tube])).iter().any(|i| i.contains("/layers/0/volumetric")));
    assert!(
        issues(json!([spot("s", json!({ "volumetric": { "density": 0.5, "samples": 32 } }))])).is_empty()
    );
    assert!(scene(json!([spot("s", json!({ "volumetric": {} }))])).features().contains("volumetric_lights"));
}

#[test]
fn spots_project_pictures_videos_or_layers_without_loops() {
    let fine = [json!("wall"), json!("clip"), json!({ "layer": "w" })];
    for t in fine {
        let found = issues(json!([wall("w", false), spot("s", json!({ "texture": t }))]));
        assert!(found.is_empty(), "{t}: {found:?}");
    }
    let lit = issues(json!([wall("w", true), spot("s", json!({ "texture": { "layer": "w" } }))]));
    assert!(lit.iter().any(|i| i.contains("/layers/1/texture/layer")), "{lit:?}");
    let group = json!({ "id": "g", "type": "group", "layers": [wall("inner", true)] });
    assert!(!issues(json!([group, spot("s", json!({ "texture": { "layer": "g" } }))])).is_empty());
    let unknown = issues(json!([spot("s", json!({ "texture": { "layer": "nothing" } }))]));
    assert!(unknown.iter().any(|i| i.contains("nothing")), "{unknown:?}");
    assert!(!issues(json!([spot("s", json!({ "texture": "song" }))])).is_empty(), "a sound is no picture");
    let point = json!({ "id": "p", "type": "light", "texture": "wall" });
    assert!(!issues(json!([point])).is_empty(), "only spots project");
    let used = scene(json!([wall("w", false), spot("s", json!({ "texture": { "layer": "w" } }))])).features();
    assert!(used.contains("light_textures") && used.contains("layer_textures"));
}

#[test]
fn a_scene_projects_with_at_most_four_spots() {
    let spots: Vec<Json> = (0..5).map(|i| spot(&format!("s{i}"), json!({ "texture": "wall" }))).collect();
    assert!(issues(json!(spots)).iter().any(|i| i.contains("at most 4")));
    assert!(issues(json!(spots[..4])).is_empty());
}

#[test]
fn particles_take_light_and_refract_with_pictures_in_range() {
    let layer = |render: Json| json!({ "id": "p", "type": "particles", "system": "rain", "render": render });
    assert!(issues(json!([layer(json!({ "lit": true, "normal": "wall" }))])).is_empty());
    assert!(!issues(json!([layer(json!({ "normal": "song" }))])).is_empty());
    assert!(!issues(json!([layer(json!({ "refract": { "normal": "wall", "amount": 2 } }))])).is_empty());
    assert!(issues(json!([layer(json!({ "refract": { "normal": "wall", "amount": 0.4 } }))])).is_empty());
    let s = scene(json!([layer(json!({ "lit": true }))]));
    assert!(s.features().contains("particle_lighting"));
}

#[test]
fn lamps_carry_their_air_and_picture_and_drop_the_air_at_the_lowest_level() {
    let s =
        spot("s", json!({ "volumetric": { "density": 0.3, "samples": 24 }, "texture": { "layer": "w" } }));
    let runtime = Runtime::new(Arc::new(scene(json!([wall("w", false), s]))), Values::new());
    let view = [0.0, 0.0, 200.0, 100.0];
    let frame = runtime.frame(view, Quality::Q3);
    assert_eq!(frame.lamps[0].air, Some([0.3, 24.0, 0.0]));
    assert_eq!(frame.lamps[0].picture, Some(Picture::Layer("w")));
    assert_eq!(runtime.frame(view, Quality::Q0).lamps[0].air, None);
    let shown = runtime.frame(view, Quality::Q3);
    assert!(shown.draws.iter().any(|d| d.layer.id == "w"));
    let hidden = json!({ "id": "w", "type": "image", "image": "wall", "visible": false });
    let s = spot("s", json!({ "texture": { "layer": "w" } }));
    let runtime = Runtime::new(Arc::new(scene(json!([hidden, s]))), Values::new());
    let frame = runtime.frame(view, Quality::Q3);
    assert!(
        frame.draws.iter().any(|d| d.layer.id == "w" && d.hidden),
        "a hidden projected layer is still drawn"
    );
    assert_eq!(Quality::Q0.air(Tier::T3), None);
    assert_eq!(Quality::Q1.air(Tier::T3), Some((4.0, 0.5)));
    assert_eq!(Quality::Q3.air(Tier::T0), Some((4.0, 1.0)));
    assert_eq!((Quality::Q3.bend(), Quality::Q2.bend(), Quality::Q1.bend()), (Some(1.0), Some(0.5), None));
}
