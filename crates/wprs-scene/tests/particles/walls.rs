use super::*;

fn walled(collide: Json, shape: Json) -> Json {
    with(
        sparks(),
        json!({ "emitters": [{ "shape": shape, "rate": 200 }],
            "init": { "life": [4, 4], "speed": [40, 80], "angle": [0, 360], "size": [2, 2] },
            "forces": [], "collide": collide }),
    )
}

fn alive(system: Json, frames: usize) -> Vec<Particle> {
    run(&scene(system, true), frames, Quality::Q3).particles.into_iter().filter(|p| p.age < p.life).collect()
}

#[test]
fn boxes_keep_particles_inside_or_out() {
    let inside = json!([{ "bounds": { "box": { "from": [50, 20], "to": [150, 80], "inside": true } }, "bounce": 0.5 }]);
    let all = alive(walled(inside, json!({ "point": [100, 50] })), 90);
    assert!(
        all.len() > 50
            && all.iter().all(|p| (49.9..=150.1).contains(&p.pos[0]) && (19.9..=80.1).contains(&p.pos[1]))
    );
    let out = json!([{ "bounds": { "box": { "from": [80, 30], "to": [120, 70] } } }]);
    let all = alive(walled(out, json!({ "box": [[0, 0], [200, 100]] })), 90);
    let within = |p: &&Particle| (80.5..119.5).contains(&p.pos[0]) && (30.5..69.5).contains(&p.pos[1]);
    assert_eq!(all.iter().filter(within).count(), 0);
}

#[test]
fn circles_with_inside_hold_particles_and_segments_block_them() {
    let ring = json!([{ "bounds": { "circle": { "center": [100, 50], "radius": 30, "inside": true } } }]);
    let all = alive(walled(ring, json!({ "point": [100, 50] })), 90);
    assert!(all.iter().all(|p| (p.pos[0] - 100.0).hypot(p.pos[1] - 50.0) <= 30.01), "{all:?}");
    let wall = json!([{ "bounds": { "segment": { "from": [60, -900], "to": [60, 900] } }, "bounce": 1 }]);
    let all = alive(walled(wall, json!({ "point": [40, 50] })), 90);
    assert!(all.len() > 50 && all.iter().all(|p| p.pos[0] < 60.0), "{all:?}");
}

#[test]
fn sticking_particles_stay_where_they_hit() {
    let floor = json!([{ "bounds": { "plane": { "point": [0, 70], "normal": [0, -1] } }, "stick": true }]);
    let mut s = walled(floor, json!({ "point": [100, 20] }));
    s["forces"] = json!([{ "gravity": [0, 400] }]);
    let early = alive(s.clone(), 30);
    let later = alive(s, 60);
    let stuck: Vec<&Particle> = later.iter().filter(|p| p.spare[0] > 0.5).collect();
    assert!(!stuck.is_empty() && stuck.iter().all(|p| (p.pos[1] - 70.0).abs() < 1e-3 && p.vel == [0.0; 2]));
    for p in early.iter().filter(|p| p.spare[0] > 0.5) {
        let now = later.iter().find(|q| q.id == p.id).unwrap();
        assert_eq!(now.pos, p.pos);
    }
}

#[test]
fn colliders_that_kill_start_death_children() {
    let mut doc: Json = serde_json::from_str(&scene(
        walled(json!([{ "bounds": "canvas", "kill": true }]), json!({ "point": [100, 50] })),
        true,
    ))
    .unwrap();
    doc["particles"]["dots"]["children"] = json!([{ "system": "puff", "on": "death" }]);
    doc["particles"]["puff"] = json!({ "capacity": 400, "emitters": [{ "shape": { "point": [0, 0] }, "burst": { "count": 2 } }],
        "init": { "life": [5, 5], "speed": [0, 0] } });
    let mut rt = runtime(&doc.to_string());
    let bake = Bake::new(&rt.scene, 0, Tier::T3, true, |_| None).unwrap();
    let mut clouds: Vec<Cloud> = bake.allocs.iter().map(|a| Cloud::new(*a)).collect();
    let (mut sims, mut stamp) = (vec![Sim::default(); bake.allocs.len()], 0);
    for _ in 0..60 {
        rt.advance(1.0 / 30.0);
        rt.frame(VIEW, Quality::Q3);
        let swarm = rt.swarm(0).unwrap();
        if std::mem::replace(&mut stamp, swarm.stamp) != swarm.stamp {
            for j in 0..swarm.count() {
                (0..sims.len()).for_each(|p| sims[p] = bake.apply(p, swarm.sim(j, p)));
                Cloud::brood(&mut clouds, &sims, &bake.hosts, &bake.points);
            }
        }
    }
    let puffs: Vec<&Particle> = clouds[1].particles.iter().filter(|p| p.age < p.life).collect();
    assert!(puffs.len() > 20, "{}", puffs.len());
    assert!(puffs.iter().all(|p| (-3.0..203.0).contains(&p.pos[0]) && (-3.0..103.0).contains(&p.pos[1])));
}

#[test]
fn layer_colliders_cannot_hold_their_own_system() {
    let mut doc: Json = serde_json::from_str(&scene(sparks(), true)).unwrap();
    doc["layers"] = json!([{ "id": "g", "type": "group", "layers": [{ "id": "p", "type": "particles", "system": "dots" }] },
        { "id": "sky", "type": "solid", "color": "#ffffff" }]);
    doc["particles"]["dots"]["collide"] = json!([{ "bounds": { "layer": { "id": "g" } } }]);
    let found = Scene::from_json(&doc.to_string()).unwrap().validate(&props());
    assert!(found.contains(&Issue::Circle("/particles/dots/collide/0/bounds/layer/id".into())), "{found:?}");
    doc["particles"]["dots"]["collide"][0]["bounds"]["layer"]["id"] = json!("sky");
    let fine = Scene::from_json(&doc.to_string()).unwrap();
    assert_eq!(fine.validate(&props()), []);
    assert!(fine.features().contains("particle_collisions"));
}
