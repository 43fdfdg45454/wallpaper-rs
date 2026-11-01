use super::*;

fn deep(children: Json) -> Json {
    still(json!({ "capacity": 4, "perspective": { "focal": 100 },
        "init": { "life": [0.1, 0.1], "speed": [0, 0], "size": [4, 4], "depth": [100, 100] },
        "emitters": [{ "shape": { "point": [160, 50] }, "burst": { "count": 1 } }], "children": children }))
}

fn kid(more: Json) -> Json {
    still(json!({ "capacity": 8, "emitters": [{ "shape": { "point": [0, 0] }, "burst": { "count": 1 } }],
        "init": { "life": [5, 5], "speed": [0, 0], "size": [4, 4] } }))
    .as_object()
    .unwrap()
    .iter()
    .chain(more.as_object().unwrap())
    .map(|(k, v)| (k.clone(), v.clone()))
    .collect()
}

#[test]
fn children_of_a_deep_parent_are_born_where_it_is_drawn_or_at_its_depth() {
    let systems = json!({ "a": deep(json!([{ "system": "b", "on": "death" }, { "system": "c", "on": "death" }])),
        "b": kid(json!({})), "c": kid(json!({ "perspective": { "focal": 100 }, "init": { "life": [5, 5], "speed": [0, 0], "size": [4, 4], "depth": [20, 20] } })) });
    let mut fly = Fly::of(systems);
    fly.run(0.4);
    let (flat, far) = (fly.alive(1), fly.alive(2));
    assert!(flat.len() == 1 && far.len() == 1, "{flat:?} {far:?}");
    assert!(near(flat[0].pos, [130.0, 50.0], 0.01), "drawn at half the way to the middle: {flat:?}");
    assert!((flat[0].size - 2.0).abs() < 1e-4, "drawn at half its size: {flat:?}");
    assert!(near(far[0].pos, [160.0, 50.0], 0.01) && (far[0].depth[0] - 120.0).abs() < 1e-3, "{far:?}");
}

#[test]
fn a_flat_parent_leaves_its_children_where_they_always_were() {
    let flat = still(json!({ "capacity": 4, "init": { "life": [0.1, 0.1], "speed": [0, 0] },
        "emitters": [{ "shape": { "point": [160, 50] }, "burst": { "count": 1 } }],
        "children": [{ "system": "b", "on": "death" }] }));
    let mut fly = Fly::of(json!({ "a": flat, "b": kid(json!({})) }));
    fly.run(0.4);
    let kids = fly.alive(1);
    assert!(kids.len() == 1 && kids[0].pos == [160.0, 50.0] && kids[0].size == 4.0, "{kids:?}");
}

#[test]
fn children_born_from_their_parent_cannot_emit_from_a_layer() {
    let emits = json!({ "capacity": 8, "emitters": [{ "shape": { "layer": { "id": "bg" } }, "rate": 10 }] });
    let issues = |on: &str| {
        let parent = still(json!({ "children": [{ "system": "b", "on": on }] }));
        let layers = json!([{ "id": "bg", "type": "solid", "color": "#ffffff", "size": [20, 20] },
            { "id": "p", "type": "particles", "system": "a" }]);
        let text = json!({ "schema": SCHEMA, "canvas": { "size": [200, 100] }, "layers": layers,
            "particles": { "a": parent, "b": emits } });
        Scene::from_json(&text.to_string()).unwrap().validate(&BTreeMap::new())
    };
    for on in ["spawn", "death", "follow"] {
        let found = issues(on);
        assert!(
            found.iter().any(|i| matches!(i, Issue::Hatched(p) if p.ends_with("/children/0/system"))),
            "{on}: {found:?}"
        );
    }
    assert!(!issues("static").iter().any(|i| matches!(i, Issue::Hatched(_))));
}

#[test]
fn boids_born_at_one_point_spread_apart() {
    let flock =
        json!({ "boids": { "radius": 30, "separation": 4, "alignment": 0, "cohesion": 0, "max_speed": 60 } });
    let mut fly = Fly::of(json!({ "a": still(json!({ "capacity": 64, "forces": [flock] })) }));
    fly.run(1.0);
    let all = fly.alive(0);
    assert_eq!(all.len(), 50);
    let closest = all.iter().enumerate().flat_map(|(i, p)| {
        all[i + 1..].iter().map(move |q| (p.pos[0] - q.pos[0]).hypot(p.pos[1] - q.pos[1]))
    });
    let closest = closest.fold(f32::MAX, f32::min);
    assert!(closest > 0.5, "the flock stayed stacked: {closest}");
}
