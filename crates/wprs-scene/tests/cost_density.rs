use wprs_scene::{Quality, SCHEMA, Scene};

fn scene(effects: bool) -> Scene {
    let fx = if effects { serde_json::json!([{ "effect": "waves" }]) } else { serde_json::json!([]) };
    let text = serde_json::json!({ "schema": SCHEMA, "canvas": { "size": [1000, 500], "fit": "fill" },
        "assets": { "a": { "path": "a.png" } },
        "layers": [{ "id": "a", "type": "image", "image": "a", "size": [1000, 500], "effects": fx }] });
    Scene::from_json(&text.to_string()).unwrap()
}

#[test]
fn a_magnified_scene_counts_its_layer_effects_at_the_density_of_their_pictures() {
    let (s, bare) = (scene(true), scene(false));
    assert_eq!(s.magnify((2000, 1000)), 2.0);
    assert!((s.magnify((500, 1000)) - 2.0).abs() < 1e-4);
    let base = bare.cost_at(Quality::Q3, 1.0);
    assert_eq!(bare.cost_at(Quality::Q3, 2.0), base);
    let (near, far) = (s.cost_at(Quality::Q3, 1.0) - base, s.cost_at(Quality::Q3, 2.0) - base);
    assert!(far > 0.0 && far < near * 0.3, "{far} against {near}");
    assert!(s.cost_at(Quality::Q2, 2.0) - bare.cost_at(Quality::Q2, 2.0) < far);
    assert_eq!(s.cost(Quality::Q3), near + base);
}
