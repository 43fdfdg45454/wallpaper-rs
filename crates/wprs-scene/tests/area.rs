use serde_json::{Value, json};
use wprs_scene::{MAX_STATEFUL, SCHEMA, Scene, Tier, covered, over};

const VIEW: [f32; 4] = [0.0, 0.0, 1000.0, 1000.0];

fn scene(layers: Value) -> Scene {
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [1000, 1000] }, "layers": layers });
    Scene::from_json(&text.to_string()).unwrap()
}

fn area(scene: &Scene, id: &str) -> f32 {
    let found = scene.bounds().into_iter().find(|(l, _)| l.id == id).unwrap();
    covered(found.1, VIEW)
}

fn solid(id: &str, size: [f32; 2], position: [f32; 2]) -> Value {
    json!({ "id": id, "type": "solid", "color": "#ffffff", "size": size, "transform": { "position": position } })
}

#[test]
fn adjustment_layers_cost_the_whole_target_whatever_size_they_declare() {
    let adjust =
        json!({ "id": "adj", "type": "adjustment", "size": [1, 1], "effects": [{ "effect": "tint" }] });
    let group =
        json!({ "id": "g", "type": "group", "layers": [solid("s", [10.0, 10.0], [5.0, 5.0]), adjust] });
    let s = scene(json!([group]));
    assert_eq!((area(&s, "adj"), area(&s, "g")), (1.0, 1.0));
    let tiny = scene(
        json!([{ "id": "a", "type": "adjustment", "size": [1, 1], "effects": [{ "effect": "blur" }] }]),
    );
    let full = scene(json!([{ "id": "a", "type": "adjustment", "effects": [{ "effect": "blur" }] }]));
    assert_eq!(tiny.cost(wprs_scene::Quality::Q3), full.cost(wprs_scene::Quality::Q3));
}

#[test]
fn groups_cost_their_bounding_extent_and_layers_what_the_canvas_shows() {
    let corners = [solid("a", [10.0, 10.0], [5.0, 5.0]), solid("b", [10.0, 10.0], [995.0, 995.0])];
    let s = scene(json!([{ "id": "g", "type": "group", "layers": corners }]));
    assert_eq!(area(&s, "g"), 1.0);
    assert!((area(&s, "a") - 1e-4).abs() < 1e-6);
    let s = scene(json!([
        solid("big", [4000.0, 4000.0], [500.0, 500.0]),
        solid("corner", [100.0, 100.0], [0.0, 0.0])
    ]));
    assert_eq!(area(&s, "big"), 1.0);
    assert!((area(&s, "corner") - 0.0025).abs() < 1e-6);
    let turned = json!({ "id": "t", "type": "solid", "color": "#ffffff", "size": [100, 100],
        "transform": { "position": [500, 500], "rotation": 45 } });
    let diagonal = 2.0f32.sqrt() * 100.0;
    assert!((area(&scene(json!([turned])), "t") - diagonal * diagonal / 1e6).abs() < 1e-4, "t");
}

#[test]
fn layers_that_can_move_or_grow_cost_the_whole_canvas() {
    let bound = json!({ "id": "b", "type": "solid", "color": "#ffffff", "size": [10, 10], "transform": { "scale": { "prop": "s" } } });
    let reach = json!({ "id": "r", "type": "solid", "color": "#ffffff", "size": [10, 10],
        "effects": [{ "effect": "blur", "params": { "radius": 45 } }] });
    let s = scene(json!([bound, reach, { "id": "p", "type": "particles", "system": "none" }]));
    assert_eq!(area(&s, "b"), 1.0);
    assert!((area(&s, "r") - 0.01).abs() < 1e-6);
    assert_eq!(area(&s, "p"), 1.0);
    let mut scripted = scene(json!([solid("a", [10.0, 10.0], [5.0, 5.0])]));
    assert!(area(&scripted, "a") < 1e-3);
    scripted.scripts = serde_json::from_value(json!([{ "path": "move.js" }])).unwrap();
    assert_eq!(area(&scripted, "a"), 1.0);
}

#[test]
fn the_scene_budget_leaves_out_the_uses_past_the_limit_in_order() {
    let uses: Vec<Value> = (0..16).map(|_| json!({ "effect": "tint" })).collect();
    let layers: Vec<Value> = (0..250)
        .map(|i| json!({ "id": format!("a{i}"), "type": "adjustment", "size": [1, 1], "effects": uses }))
        .collect();
    let s = scene(json!(layers));
    let spends = s.spends();
    assert_eq!(spends.len(), 4000);
    let at = |tier: Tier, pixels: f32| over(&spends, |_| false, tier, VIEW, pixels);
    let (small, large) = (at(Tier::T0, 64.0 * 32.0), at(Tier::T0, 3840.0 * 2160.0));
    assert!(small.is_empty(), "{}", small.len());
    let kept = 4000 - large.len();
    let work = spends[0].work(VIEW, 3840.0 * 2160.0);
    assert!(kept > 0 && kept as f32 * work <= Tier::T0.ceiling() && large[0].0 == kept, "{kept}");
    assert!(large.iter().all(|(_, why)| why.contains("units of work a frame")));
    assert!(at(Tier::T3, 3840.0 * 2160.0).len() < large.len());
    assert!(s.left_over(Tier::T0, VIEW, 3840.0 * 2160.0)[0].starts_with("the effect tint on a"));
    let trimmed = s.without(large.iter().map(|(i, _)| &spends[*i]));
    assert_eq!(trimmed.spends().len(), kept);
    assert_eq!(MAX_STATEFUL, 16);
}
