use super::{props, values};
use serde_json::json;
use std::sync::Arc;
use wprs_scene::{Leveling, Quality, Runtime, SCHEMA, Scene};

fn leveling() -> Leveling {
    Leveling::of(&props(json!({
        "rate": { "type": "slider", "min": 0, "max": 100, "step": 1, "default": 80, "quality": { "q2": 50, "q0": 10 } },
        "mode": { "type": "combo", "options": ["full", "lite", "off"], "default": "full", "quality": { "q1": "lite", "q0": "off" } },
        "glow": { "type": "bool", "default": true, "quality": { "q1": false } },
        "tint": { "type": "color", "default": "#ffffff" }
    })))
}

#[test]
fn quality_levels_cap_sliders_and_pick_combos_and_flags_on_their_level_and_below() {
    let chosen = values(json!({ "rate": 80, "mode": "full", "glow": true, "tint": "#ffffff" }));
    let at = |q| leveling().tuned(&chosen, q);
    assert_eq!(at(Quality::Q3), chosen, "the highest level keeps what the user chose");
    assert_eq!(
        at(Quality::Q2),
        values(json!({ "rate": 50, "mode": "full", "glow": true, "tint": "#ffffff" }))
    );
    assert_eq!(
        at(Quality::Q1),
        values(json!({ "rate": 50, "mode": "lite", "glow": false, "tint": "#ffffff" }))
    );
    assert_eq!(
        at(Quality::Q0),
        values(json!({ "rate": 10, "mode": "off", "glow": false, "tint": "#ffffff" }))
    );
    let low = values(json!({ "rate": 20, "mode": "full", "glow": true, "tint": "#ffffff" }));
    assert_eq!(
        leveling().tuned(&low, Quality::Q1)["rate"],
        json!(20),
        "a slider below the cap keeps its value"
    );
}

#[test]
fn properties_without_quality_values_leave_the_scene_untouched() {
    let none =
        Leveling::of(&props(json!({ "rate": { "type": "slider", "min": 0, "max": 1, "default": 1 } })));
    assert!(none.is_empty());
    let chosen = values(json!({ "rate": 1 }));
    assert_eq!(none.tuned(&chosen, Quality::Q0), chosen);
}

#[test]
fn a_new_quality_level_changes_the_values_the_scene_reads_only_when_it_tunes_them() {
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [64, 64] }, "layers": [] }).to_string();
    let scene = Arc::new(Scene::from_json(&text).unwrap());
    let chosen = values(json!({ "rate": 80, "mode": "full", "glow": true, "tint": "#ffffff" }));
    let mut runtime = Runtime::new(scene.clone(), chosen.clone());
    assert!(!runtime.set_leveling(leveling()), "Q3 keeps the chosen values");
    assert!(runtime.set_quality(Quality::Q1));
    assert_eq!(runtime.props()["mode"], json!("lite"));
    assert!(!runtime.set_quality(Quality::Q1), "the same level changes nothing");
    assert!(
        runtime.set_props(values(json!({ "rate": 30, "mode": "full", "glow": true, "tint": "#ffffff" })))
    );
    assert_eq!((runtime.props()["rate"].clone(), runtime.props()["glow"].clone()), (json!(30), json!(false)));
    assert!(runtime.set_quality(Quality::Q3));
    assert_eq!(runtime.props()["glow"], json!(true));
    let mut plain = Runtime::new(scene, chosen);
    assert!(!plain.set_quality(Quality::Q0), "a scene without quality values never changes with the level");
}
