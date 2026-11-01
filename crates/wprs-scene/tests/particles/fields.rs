use super::*;

fn shaped(shape: Json, init: Json) -> Json {
    let mut s = with(
        sparks(),
        json!({ "emitters": [{ "shape": shape, "rate": 120 }], "forces": [], "collide": [],
        "points": [{ "at": [0, 0] }, { "at": [150, 60] }] }),
    );
    s["init"] = with(json!({ "life": [5, 5], "speed": [0, 0], "size": [4, 4] }), init);
    s
}

fn alive(system: Json, frames: usize) -> Vec<Particle> {
    let cloud = run(&scene(system, true), frames, Quality::Q3);
    let mut all: Vec<Particle> = cloud.particles.into_iter().filter(|p| p.age < p.life).collect();
    all.sort_by_key(|p| p.id);
    all
}

#[test]
fn births_go_around_a_point_in_turn_and_back_with_mirror() {
    let ring =
        json!({ "around": { "at": [100, 50], "count": 4, "radius": 10, "arc": 90, "mode": "mirror" } });
    let all = alive(shaped(ring, json!({})), 10);
    assert!(all.len() >= 8, "{}", all.len());
    let angle = |p: &Particle| (p.pos[1] - 50.0).atan2(p.pos[0] - 100.0).to_degrees().round() as i32;
    let turns: Vec<i32> = all.iter().take(8).map(angle).collect();
    assert_eq!(turns, [0, 30, 60, 90, 60, 30, 0, 30], "{all:?}");
    assert!(all.iter().all(|p| ((p.pos[0] - 100.0).hypot(p.pos[1] - 50.0) - 10.0).abs() < 1e-3));
}

#[test]
fn births_between_two_points_follow_the_arc_and_the_taper() {
    let line = json!({ "between": { "from": "p0", "to": "p1", "count": 5, "taper": { "position": 0.5, "size": 0.25 } } });
    let all = alive(shaped(line, json!({})), 10);
    let along: Vec<f32> = all.iter().take(5).map(|p| p.pos[0] / 150.0).collect();
    assert!(along.iter().zip([0.0, 0.25, 0.5, 0.75, 1.0]).all(|(a, b)| (a - b).abs() < 1e-4), "{along:?}");
    let sizes: Vec<f32> = all.iter().take(5).map(|p| p.size).collect();
    assert_eq!(sizes, [1.0, 2.5, 4.0, 2.5, 1.0]);
    let bow =
        json!({ "between": { "from": "p0", "to": "p1", "count": 3, "arc": { "amount": 0.5, "side": 1 } } });
    let middle = alive(shaped(bow, json!({})), 10)[1];
    let off = (middle.pos[0] - 75.0).hypot(middle.pos[1] - 30.0);
    assert!((off - 0.5 * 150f32.hypot(60.0)).abs() < 1e-2, "{middle:?}");
}

#[test]
fn remaps_map_inputs_onto_starting_values_in_order() {
    let remap = json!([{ "input": "random", "in": [0, 1], "output": "size", "out": [1, 8] },
        { "input": "distance:p1", "in": [0, 200], "clamp_in": true, "output": "alpha", "out": [1, 0] },
        { "input": "life", "in": [0, 10], "output": "size", "out": [0, 2], "op": "mul" }]);
    let all = alive(shaped(json!({ "point": [150, 60] }), json!({ "remap": remap })), 10);
    assert!(!all.is_empty());
    for p in &all {
        assert!((0.5..=8.0).contains(&p.size) && (p.color[3] - 1.0).abs() < 1e-4, "{p:?}");
    }
    assert!(
        all.iter().map(|p| p.size).fold(0.0, f32::max) - all.iter().map(|p| p.size).fold(9.0, f32::min) > 2.0
    );
}

#[test]
fn a_layer_that_holds_the_system_cannot_be_its_source() {
    let mut doc: Json = serde_json::from_str(&scene(sparks(), true)).unwrap();
    doc["layers"] = json!([{ "id": "g", "type": "group", "layers": [{ "id": "p", "type": "particles", "system": "dots" }] }]);
    doc["particles"]["dots"]["emitters"] = json!([{ "shape": { "layer": { "id": "g" } }, "rate": 10 }]);
    let found = Scene::from_json(&doc.to_string()).unwrap().validate(&props());
    assert!(found.contains(&Issue::Circle("/particles/dots/emitters/0/shape/layer/id".into())), "{found:?}");
    doc["layers"][0]["layers"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "id": "sky", "type": "solid", "color": "#ffffff" }));
    doc["particles"]["dots"]["emitters"][0]["shape"]["layer"]["id"] = json!("sky");
    let fine = Scene::from_json(&doc.to_string()).unwrap();
    assert_eq!(fine.validate(&props()), []);
    assert!(fine.features().contains("particle_layers"));
}

#[test]
fn bursts_around_points_settle_once_their_particles_are_gone() {
    let mut s = shaped(json!({ "around": { "count": 8, "radius": 10 } }), json!({ "life": [0.2, 0.2] }));
    s["emitters"][0] = json!({ "shape": s["emitters"][0]["shape"], "burst": { "count": 16 } });
    let mut rt = runtime(&scene(s, true));
    let mut wake = Wake::Frame;
    for _ in 0..30 {
        rt.advance(1.0 / 30.0);
        wake = rt.frame(VIEW, Quality::Q3).wake;
    }
    assert_eq!(wake, Wake::Idle);
}
