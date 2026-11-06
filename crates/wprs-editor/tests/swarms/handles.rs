use super::*;

fn near(a: &Value, b: [f64; 2]) -> bool {
    (a[0].as_f64().unwrap() - b[0]).abs() < 0.02 && (a[1].as_f64().unwrap() - b[1]).abs() < 0.02
}

#[test]
fn handles_place_shapes_and_points_where_the_preview_draws_them() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    edit(&mut p, "point", json!({ "point": 1, "at": [100, 50], "angle": 90 })).unwrap();
    let around = json!({ "around": { "at": "p1", "count": 4, "radius": 10, "arc": 90 } });
    edit(&mut p, "shape", json!({ "emitter": 0, "shape": around })).unwrap();
    let line = json!({ "between": { "from": [0, 0], "to": [100, 0], "count": 3, "arc": { "amount": 0.5, "side": 1 },
        "taper": { "position": 0.5, "size": 0.2 } } });
    edit(&mut p, "shape", json!({ "emitter": 1, "shape": line })).unwrap();
    let h = handles(&p);
    assert_eq!(h["points"][1]["name"], "p1");
    assert!(near(&h["points"][1]["at"], [100.0, 50.0]), "{}", h["points"][1]);
    let ring = &h["emitters"][0];
    assert_eq!((ring["kind"].clone(), ring["count"].clone()), (json!("around"), json!(4)));
    assert!(near(&ring["places"][0], [100.0, 60.0]) && near(&ring["places"][3], [90.0, 50.0]), "{ring}");
    assert!(near(&ring["ends"][1], [90.0, 50.0]));
    let bow = &h["emitters"][1];
    assert!(near(&bow["bulge"], [50.0, 50.0]), "{bow}");
    let sizes: Vec<f64> =
        bow["places"].as_array().unwrap().iter().map(|p| p["size"].as_f64().unwrap()).collect();
    assert_eq!(sizes, [0.2, 1.0, 0.2]);
    act(&mut p, json!({ "set": { "pointer": "/scene/layers/1/transform", "value": { "position": [1060, 540], "rotation": 90 } } })).unwrap();
    let moved = handles(&p);
    assert!(
        near(&moved["points"][1]["at"], [1060.0 + 540.0 - 50.0, 540.0 - 960.0 + 100.0]),
        "{}",
        moved["points"][1]
    );
    let inverse: Vec<f64> = serde_json::from_value(moved["inverse"].clone()).unwrap();
    let [x, y] =
        [moved["points"][1]["at"][0].as_f64().unwrap(), moved["points"][1]["at"][1].as_f64().unwrap()];
    let back = [inverse[0] * x + inverse[2] * y + inverse[4], inverse[1] * x + inverse[3] * y + inverse[5]];
    assert!((back[0] - 100.0).abs() < 0.05 && (back[1] - 50.0).abs() < 0.05, "{back:?}");
}

#[test]
fn layer_sources_list_only_layers_that_may_feed_the_system_and_show_their_pixels() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    act(&mut p, json!({ "group": { "ids": ["flakes"], "id": "pack" } })).unwrap();
    let h = handles(&p);
    let sources: Vec<&str> = h["sources"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
    assert!(sources.contains(&"logo") && sources.contains(&"sky"), "{sources:?}");
    assert!(!sources.contains(&"pack") && !sources.contains(&"flakes"), "{sources:?}");
    edit(&mut p, "layer", json!({ "emitter": 0, "id": "logo", "threshold": 0.5 })).unwrap();
    let shown = &handles(&p)["emitters"][0];
    let samples = shown["samples"].as_array().unwrap();
    assert!(!samples.is_empty() && shown.get("approximate").is_none(), "{shown}");
    assert!(
        samples.iter().all(|s| (940.0..961.0).contains(&s[0].as_f64().unwrap())),
        "only the opaque left half"
    );
    assert!(edit(&mut p, "layer", json!({ "emitter": 0, "id": "pack" })).is_err());
}

#[test]
fn pulls_show_their_center_and_rings_where_the_preview_draws_them() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    edit(&mut p, "point", json!({ "point": 0, "at": [100, 50] })).unwrap();
    let forces = [
        json!({ "attractor": { "target": "p0", "strength": 30, "radius": 40 } }),
        json!({ "vortex": { "center": [10, 20], "strength": 5, "radius": 80, "inner": 5, "outer": 60,
            "ring": { "radius": 30, "width": 4, "pull": 2 } } }),
        json!({ "orbit": { "center": "cursor", "strength": 10 } }),
        json!({ "slow_near": { "target": [0, 0], "inner": 10, "outer": 20, "factor": 0.5 } }),
    ];
    for force in forces {
        edit(&mut p, "force", json!({ "force": force })).unwrap();
    }
    let h = handles(&p);
    let pulls = h["pulls"].as_array().unwrap();
    let kinds: Vec<&str> = pulls.iter().map(|e| e["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["attractor", "vortex", "orbit", "slow_near"]);
    assert!(near(&pulls[0]["at"], [100.0, 50.0]) && pulls[0]["ref"] == "p0", "{}", pulls[0]);
    let ring = &pulls[0]["rings"][0];
    assert!(ring["key"] == "radius" && near(&ring["at"], [100.0, 10.0]), "{ring}");
    let keys: Vec<&str> =
        pulls[1]["rings"].as_array().unwrap().iter().map(|r| r["key"].as_str().unwrap()).collect();
    assert_eq!(keys, ["radius", "inner", "outer", "ring"]);
    assert!(pulls[2]["at"].is_null() && pulls[2]["field"] == "center");
    assert_eq!(pulls[3]["factor"], 0.5);
}

#[test]
fn a_particles_layer_shows_its_own_control_points() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    edit(&mut p, "point", json!({ "point": 0, "at": [100, 50] })).unwrap();
    let layer = wprs_editor::swarms::handles(&p.dir, &p.doc, "flakes", None, 0.0)["layer"].clone();
    let id = layer.as_str().unwrap().to_owned();
    let own =
        json!({ "layer": { "instance": { "layer": id, "values": { "points": [[10, 20], [30, 40]] } } } });
    act(&mut p, own).unwrap();
    let h = wprs_editor::swarms::handles(&p.dir, &p.doc, "flakes", Some(&id), 0.0);
    let points = h["points"].as_array().unwrap();
    assert_eq!(points.len(), 2, "{points:?}");
    assert_eq!((&points[0]["local"], &points[0]["own"]), (&json!([10.0, 20.0]), &json!(true)));
    assert_eq!(points[1]["local"], json!([30.0, 40.0]));
    assert_eq!(handles(&p)["points"][0]["local"], json!([100.0, 50.0]), "the system keeps its own point");
}

#[test]
fn turbulent_rays_follow_the_seed_of_the_scene_and_the_preview_runs_with_it() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let gust = json!({ "forward": 0, "scale": 0.5, "offset": 2, "speed": [10, 20] });
    edit(&mut p, "turbulent", json!({ "turbulent": gust })).unwrap();
    let rays = |p: &Project| handles(p)["fields"]["turbulent"]["samples"].clone();
    act(&mut p, json!({ "set": { "pointer": "/scene/clock/seed", "value": 9 } })).unwrap();
    let nine = rays(&p);
    assert_eq!(handles(&p)["seed"], json!(9));
    let t: wprs_scene::Turbulent = serde_json::from_value(gust).unwrap();
    let salt = wprs_scene::particle_salt(system(&p)["seed"].as_u64().unwrap_or(0), 9, 0, 0);
    let first = f64::from(wprs_scene::turbulent_ray(&t, salt, 0, 0.0));
    assert!((nine[0].as_f64().unwrap() - first).abs() < 0.01, "{nine} {first}");
    act(&mut p, json!({ "set": { "pointer": "/scene/clock/seed", "value": 10 } })).unwrap();
    assert_ne!(rays(&p), nine, "another seed turns the rays");
    act(&mut p, json!({ "set": { "pointer": "/scene/clock/seed", "value": null } })).unwrap();
    let fresh = wprs_editor::swarms::preview_seed();
    assert_eq!(handles(&p)["seed"], json!(fresh));
    let mut scene = p.doc["scene"].clone();
    wprs_editor::swarms::pin_seed(&mut scene);
    assert_eq!(scene["clock"]["seed"], json!(fresh), "the preview starts with the seed the rays use");
    let mut fixed = json!({ "clock": { "seed": 3 } });
    wprs_editor::swarms::pin_seed(&mut fixed);
    assert_eq!(fixed["clock"]["seed"], json!(3));
}
