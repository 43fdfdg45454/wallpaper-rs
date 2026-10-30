use super::*;

fn sparks(more: Json) -> Json {
    let mut s = json!({ "capacity": 400, "emitters": [{ "shape": { "circle": { "center": [0, 0], "radius": 5 } }, "burst": { "count": 10 } }],
        "init": { "life": [5, 5], "speed": [0, 0] } });
    more.as_object().unwrap().iter().for_each(|(k, v)| s[k] = v.clone());
    s
}

#[test]
fn children_burst_where_their_parents_die() {
    let parent = still(
        json!({ "capacity": 20, "init": { "life": [0.2, 0.2], "speed": [0, 0], "color": ["#00ff00", "#00ff00"] },
        "emitters": [{ "shape": { "point": [50, 50] }, "burst": { "count": 5 } }],
        "children": [{ "system": "b", "on": "death", "inherit": ["color"], "offset": [10, 0], "scale": 2 }] }),
    );
    let mut fly = Fly::of(json!({ "a": parent, "b": sparks(json!({})) }));
    assert_eq!(fly.rt.flocks()[0].parts.len(), 2);
    fly.run(0.4);
    let (parents, kids) = (fly.alive(0), fly.alive(1));
    assert!(parents.is_empty() && kids.len() == 50, "{} {}", parents.len(), kids.len());
    assert!(
        kids.iter().all(|p| near(p.pos, [60.0, 50.0], 10.01) && p.color == [0.0, 1.0, 0.0, 1.0]),
        "{kids:?}"
    );
    assert!(kids.iter().all(|p| p.size > 3.9), "children grow with scale");
    let half =
        still(json!({ "children": [{ "system": "b", "on": "spawn", "probability": 0.5, "max": 120 }] }));
    let mut fly = Fly::of(json!({ "a": half, "b": sparks(json!({})) }));
    fly.frame(0.0);
    fly.frame(1.0 / 60.0);
    let kids = fly.alive(1).len();
    assert!(kids.is_multiple_of(10) && (100..=120).contains(&kids), "{kids}");
}

#[test]
fn followers_ride_their_parents_and_die_with_them() {
    let parent =
        still(json!({ "capacity": 4, "init": { "life": [0.5, 0.5], "speed": [60, 60], "angle": [0, 0] },
        "emitters": [{ "shape": { "point": [20, 50] }, "burst": { "count": 2 } }],
        "children": [{ "system": "b", "on": "follow" }] }));
    let trail = sparks(json!({ "emitters": [{ "shape": { "point": [0, 0] }, "rate": 60 }] }));
    let mut fly = Fly::of(json!({ "a": parent, "b": trail }));
    fly.run(0.3);
    let (parents, kids) = (fly.alive(0), fly.alive(1));
    assert!(kids.len() >= 20, "{}", kids.len());
    assert!(kids.iter().all(|k| parents.iter().any(|p| near(k.pos, p.pos, 0.5))), "they ride along");
    fly.run(0.3);
    assert!(fly.alive(0).is_empty() && fly.alive(1).is_empty());
    let left =
        sparks(json!({ "space": "world", "emitters": [{ "shape": { "point": [0, 0] }, "rate": 60 }] }));
    let mut fly = Fly::of(json!({ "a": parent, "b": left }));
    fly.run(0.3);
    let xs: Vec<f32> = fly.alive(1).iter().map(|p| p.pos[0]).collect();
    assert!(
        xs.iter().any(|x| *x < 30.0) && xs.iter().any(|x| *x > 35.0),
        "a world trail stays behind: {xs:?}"
    );
}

#[test]
fn families_are_bounded() {
    let chain = |next: &str| still(json!({ "children": [{ "system": next, "on": "death" }] }));
    let systems = json!({ "a": chain("b"), "b": chain("c"), "c": chain("d"), "d": still(json!({})) });
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [200, 100] }, "layers": layer("a"), "particles": systems }).to_string();
    let scene = Scene::from_json(&text).unwrap();
    assert!(scene.validate(&BTreeMap::new()).iter().any(|i| matches!(i, Issue::Generations(_))));
    assert_eq!(flocks(&scene)[0].parts.len(), 3, "the engine keeps two generations");
    let wide = |n: usize| {
        still(
            json!({ "capacity": 10, "children": (0..4).map(|_| json!({ "system": format!("s{n}"), "on": "spawn" })).collect::<Vec<_>>() }),
        )
    };
    let mut systems = json!({ "s2": still(json!({ "capacity": 10 })) });
    (0..2).for_each(|n| systems[format!("s{n}")] = wide(n + 1));
    let layers = json!((0..4).map(|i| json!({ "id": format!("l{i}"), "type": "particles", "system": "s0", "instance": { "count": 1 } })).collect::<Vec<_>>());
    let text =
        json!({ "schema": SCHEMA, "canvas": { "size": [200, 100] }, "layers": layers, "particles": systems })
            .to_string();
    let scene = Scene::from_json(&text).unwrap();
    let issues = scene.validate(&BTreeMap::new());
    assert!(
        issues.contains(&Issue::TooMany { path: "/particles".into(), count: 84, max: MAX_PARTS }),
        "{issues:?}"
    );
}

fn crowded() -> Json {
    let point = json!({ "shape": { "point": [0, 0] }, "rate": 100000 });
    json!({
        "a": { "capacity": 5000, "emitters": [{ "shape": { "point": [100, 50] }, "rate": 100000 }],
            "init": { "life": [100, 100], "speed": [0, 0] }, "children": [{ "system": "b", "on": "follow" }] },
        "b": { "capacity": 10, "emitters": vec![point; 8], "init": { "life": [100, 100], "speed": [0, 0] } }
    })
}

#[test]
fn children_without_room_cost_nothing() {
    let mut fly = Fly::of(crowded());
    let (start, mut slowest) = (std::time::Instant::now(), std::time::Duration::ZERO);
    for _ in 0..4 {
        let at = std::time::Instant::now();
        fly.frame(1.0 / 30.0);
        slowest = slowest.max(at.elapsed());
    }
    assert!(slowest.as_millis() < 300, "{slowest:?} in {:?}", start.elapsed());
    assert!(fly.alive(0).len() > 4000 && fly.alive(1).len() == 10);
    let sim = fly.bake.apply(1, fly.rt.swarm(0).unwrap().sim(0, 1));
    assert_eq!(sim.mode[2], 10, "a child never hatches more than its room");
}
