use super::{act, light, project};
use serde_json::json;
use wprs_editor::lamps;

#[test]
fn the_lights_report_gives_the_handles_of_every_type() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    for kind in ["point", "spot", "tube", "directional"] {
        light(&mut p, json!({ "add": { "light": kind, "at": [400, 300] } })).unwrap();
    }
    let report = lamps::lights(&p.dir, &p.doc, 0.0);
    let rows = report["lights"].as_array().unwrap();
    assert_eq!(rows.len(), 4);
    let point = &rows[0];
    assert_eq!(
        (point["at"].clone(), point["z_knob"].clone()),
        (json!([400.0, 300.0]), json!([400.0, 240.0]))
    );
    assert_eq!(point["radius_knob"], json!([800.0, 300.0]));
    assert_eq!(point["box"], json!([0.0, -100.0, 800.0, 800.0]));
    let spot = &rows[1];
    assert_eq!(spot["aim"], json!([400.0, 460.0]));
    let outer = spot["outer"].as_array().unwrap();
    let [x0, x1] = [outer[0][0].as_f64().unwrap(), outer[1][0].as_f64().unwrap()];
    assert!(x0 > 400.0 && x1 < 400.0 && (x0 - 400.0 + (x1 - 400.0)).abs() < 1e-3);
    let tube = &rows[2];
    assert_eq!(tube["ends"], json!([[200.0, 300.0], [600.0, 300.0]]));
    assert_eq!(rows[3]["aim"], json!([400.0, 460.0]));
    assert_eq!(report["count"], 4);
    let tiers = report["tiers"].as_array().unwrap();
    assert_eq!((tiers[0]["cap"].clone(), tiers[3]["cap"].clone()), (json!(8), json!(12)));
}

#[test]
fn the_report_counts_lights_past_each_tier_and_plots_the_falloff() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    for i in 0..10 {
        light(&mut p, json!({ "add": { "id": format!("l{i}") } })).unwrap();
    }
    let tiers = lamps::lights(&p.dir, &p.doc, 0.0)["tiers"].clone();
    assert_eq!((tiers[1]["left_out"].clone(), tiers[2]["left_out"].clone()), (json!(2), json!(0)));
    let f = lamps::falloff(&p.dir, &p.doc, "l0", 0.0);
    let points = f["curve"]["points"].as_array().unwrap();
    assert_eq!((points[0][1].as_f64(), points.last().unwrap()[1].as_f64()), (Some(1.0), Some(0.0)));
    assert!(lamps::falloff(&p.dir, &p.doc, "logo", 0.0).is_null());
}

#[test]
fn the_lighting_cost_shows_the_bake_and_the_animated_lights() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    act(&mut p, json!({ "lighting": { "layer": "logo" } })).unwrap();
    light(&mut p, json!({ "add": { "id": "l" } })).unwrap();
    let still = lamps::cost(&p.dir, &p.doc, 0.0, wprs_scene::Quality::Q3);
    assert_eq!(still["levels"][3]["per_frame"], 0.0);
    assert_eq!(still["levels"][0]["baked"], true);
    let walk = json!({ "keys": [[0, [0, 0]], [2, [500, 500]]] });
    let pointer = format!("{}/transform/position", wprs_editor::layers::find(&p.doc, "l").unwrap());
    act(&mut p, json!({ "set": { "pointer": pointer, "value": walk } })).unwrap();
    let moving = lamps::cost(&p.dir, &p.doc, 0.0, wprs_scene::Quality::Q3);
    assert!(moving["animated"] == true && moving["levels"][3]["per_frame"].as_f64().unwrap() > 0.0);
    assert_eq!(moving["levels"][0]["per_frame"], 0.0);
    assert_eq!(moving["lit_layers"], json!(["logo"]));
}

#[test]
fn spot_and_directional_lights_show_a_tilt_knob_on_their_aim_line() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    for (id, kind) in [("p", "point"), ("s", "spot"), ("t", "tube"), ("d", "directional")] {
        light(&mut p, json!({ "add": { "id": id, "light": kind, "at": [400, 300] } })).unwrap();
    }
    let knob = |p: &wprs_editor::Project, row: usize| {
        let rows = lamps::lights(&p.dir, &p.doc, 0.0)["lights"].clone();
        rows[row]["tilt_knob"].as_array().map(|k| [k[0].as_f64().unwrap(), k[1].as_f64().unwrap()])
    };
    assert_eq!((knob(&p, 0), knob(&p, 2)), (None, None));
    let [x, y] = knob(&p, 1).unwrap();
    assert!(x == 400.0 && (y - (300.0 + 160.0 * f64::from(lamps::tilt_share(45.0)))).abs() < 0.01, "{y}");
    let [_, flat] = knob(&p, 3).unwrap();
    assert!((flat - (300.0 + 160.0 * f64::from(lamps::tilt_share(50.0)))).abs() < 0.01, "{flat}");
    light(&mut p, json!({ "set": { "layer": "s", "tilt": -60 } })).unwrap();
    let [_, low] = knob(&p, 1).unwrap();
    assert!(low > y + 60.0 && low < 460.0, "a flatter beam puts the knob nearer the end: {low}");
    assert!(lamps::tilt_share(89.0) < lamps::tilt_share(0.0) && lamps::tilt_share(-89.0) < 0.91);
}
