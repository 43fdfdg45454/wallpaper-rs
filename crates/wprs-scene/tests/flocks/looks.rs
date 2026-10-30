use super::*;

fn scene(system: Json, assets: Json) -> Scene {
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [200, 100] }, "layers": layer("a"),
        "particles": { "a": system }, "assets": assets });
    Scene::from_json(&text.to_string()).unwrap_or_else(|e| panic!("{e}"))
}

fn issues(more: Json) -> (Vec<Issue>, std::collections::BTreeSet<String>) {
    let s = scene(still(more), json!({ "floor": { "path": "floor.png" }, "song": { "path": "song.ogg" } }));
    (s.validate(&BTreeMap::new()), s.features().into_iter().map(String::from).collect())
}

#[test]
fn gradients_pick_births_along_their_stops() {
    let gradient = json!([[0, "#ff0000"], [0.5, "#00ff00"], [1, "#0000ff"]]);
    let mut fly = Fly::of(
        json!({ "a": still(json!({ "init": { "life": [10, 10], "speed": [0, 0], "gradient": gradient } })) }),
    );
    fly.frame(0.0);
    let born = fly.alive(0);
    let on_path = |c: &[f32; 4]| (c[0] + c[1] + c[2] - 1.0).abs() < 1e-3 && c[0].min(c[2]) < 1e-3;
    assert!(born.len() == 50 && born.iter().all(|p| on_path(&p.color)), "{:?}", born[0].color);
    assert!(born.iter().any(|p| p.color[0] > 0.6) && born.iter().any(|p| p.color[2] > 0.6));
}

#[test]
fn masks_stop_falling_particles_on_their_opaque_part() {
    let floor: Vec<u8> =
        (0..50 * 25).flat_map(|i| [255, 255, 255, if i / 50 >= 15 { 255 } else { 0 }]).collect();
    let system = json!({ "capacity": 100, "seed": 1,
        "emitters": [{ "shape": { "box": { "from": [20, 0], "to": [180, 10] } }, "burst": { "count": 80 } }],
        "init": { "life": [30, 30], "speed": [0, 0], "size": [2, 2] }, "forces": [{ "gravity": [0, 400] }],
        "collide": [{ "bounds": { "mask": { "texture": "floor", "threshold": 0.5 } }, "bounce": 0.3 }] });
    let s = scene(system, json!({ "floor": { "path": "floor.png" } }));
    let rt = Runtime::new(Arc::new(s), Values::new());
    let mut fly = Fly::with(rt, |id| (id == "floor").then_some((50, 25, &floor[..])));
    fly.run(2.0);
    let alive = fly.alive(0);
    assert!(alive.len() == 80 && alive.iter().all(|p| p.pos[1] > 50.0 && p.pos[1] < 62.0), "{:?}", alive[0]);
    assert!(alive.iter().all(|p| p.vel[1].abs() < 30.0), "they came to rest: {:?}", alive[0].vel);
}

#[test]
fn trails_remember_recent_positions_and_forget_the_dead() {
    let system = still(
        json!({ "capacity": 4, "emitters": [{ "shape": { "point": [10, 50] }, "burst": { "count": 1 } }],
        "init": { "life": [0.5, 0.5], "velocity": [[60, 0], [60, 0]], "size": [3, 3] },
        "render": { "trail": { "length": 6, "width": 0.5 } } }),
    );
    let mut fly = Fly::of(json!({ "a": system }));
    assert_eq!(fly.bake.trails, [6]);
    fly.run(0.2);
    let (k, cloud) = (6, &fly.clouds[0]);
    let (slot, p) = cloud.particles.iter().enumerate().find(|(_, p)| p.age < p.life).unwrap();
    let marks = &cloud.trail[slot * k..(slot + 1) * k];
    assert_eq!(marks[0], mark(p));
    let xs: Vec<f32> = marks.iter().map(|m| f32::from_bits(m[0])).collect();
    assert!(xs.windows(2).all(|w| w[0] > w[1]) && (xs[0] - xs[1] - 1.0).abs() < 1e-3, "{xs:?}");
    assert!(marks.iter().all(|m| m[2] != 0 && m[3] >> 24 == 255));
    fly.run(0.5);
    assert!(fly.clouds[0].trail.iter().all(|m| m[2] == 0), "a dead particle leaves no trail");
    let low = Bake::new(&fly.rt.scene, 0, Tier::T1, false, |_| None).unwrap();
    assert_eq!(
        (low.trails[0], Bake::new(&fly.rt.scene, 0, Tier::T0, true, |_| None).unwrap().trails[0]),
        (4, 0)
    );
}

#[test]
fn orders_put_the_chosen_particles_last() {
    let at = |age: f32, size: f32, y: f32| Particle {
        age,
        life: 1.0,
        size,
        pos: [0.0, y],
        look: [1.0; 2],
        ..Particle::default()
    };
    let all = [at(0.9, 1.0, 80.0), at(0.1, 4.0, 10.0), Particle::default(), at(0.5, 2.0, 50.0)];
    let ages = |kind| stacked(&all, kind, 4.0, 100.0).iter().map(|p| p.age).collect::<Vec<_>>();
    assert_eq!(ages(Stack::Newest), [0.9, 0.5, 0.1, 0.0]);
    assert_eq!(ages(Stack::Oldest), [0.1, 0.5, 0.9, 0.0]);
    assert_eq!(ages(Stack::Largest), [0.9, 0.5, 0.1, 0.0]);
    assert_eq!(ages(Stack::Smallest), [0.1, 0.5, 0.9, 0.0]);
    assert_eq!(ages(Stack::Lowest), [0.1, 0.5, 0.9, 0.0]);
    assert!(stacked(&all, Stack::Lowest, 4.0, 100.0)[3].life == 0.0);
}

#[test]
fn bound_birth_ranges_and_shapes_follow_their_sources() {
    let loud = json!({ "audio": "bass", "scale": 10 });
    let system = still(json!({ "capacity": 2000, "emitters": [{ "shape": { "circle": { "center": [100, 50],
        "radius": { "audio": "bass", "scale": 20 } } }, "rate": 600 }],
        "init": { "life": [10, 10], "speed": [0, 0], "size": [loud, loud] } }));
    let mut fly = Fly::of(json!({ "a": system }));
    let bass = |b| AudioFrame { bass: b, silent: false, ..AudioFrame::default() };
    fly.rt.set_audio(bass(0.7));
    fly.frame(0.0);
    fly.run(0.2);
    let first = fly.alive(0);
    assert!(
        !first.is_empty()
            && first.iter().all(|p| (p.size - 7.0).abs() < 1e-3 && near(p.pos, [100.0, 50.0], 14.01))
    );
    fly.rt.set_audio(bass(0.3));
    fly.run(0.2);
    let later: Vec<Particle> = fly.alive(0).into_iter().filter(|p| p.age < 0.15).collect();
    assert!(
        !later.is_empty()
            && later.iter().all(|p| (p.size - 3.0).abs() < 1e-3 && near(p.pos, [100.0, 50.0], 6.01))
    );
}

#[test]
fn new_looks_are_validated_and_named() {
    let (found, features) =
        issues(json!({ "init": { "life": [1, 1], "gradient": [[0, "#ff0000"], [1, "#0000ff"]] },
        "render": { "order": "newest", "trail": { "length": 8, "width": 0.5 } },
        "collide": [{ "bounds": { "mask": { "texture": "floor" } } }] }));
    assert!(
        found.is_empty() && features.contains("particles2") && !features.contains("particle_ropes"),
        "{found:?}"
    );
    let stops: Vec<Json> = (0..9).map(|i| json!([i as f32 / 8.0, "#ffffff"])).collect();
    let bad = [
        json!({ "init": { "gradient": stops } }),
        json!({ "init": { "gradient": [[0.5, "#ffffff"], [0.2, "#000000"]] } }),
        json!({ "init": { "gradient": [[0, "#ffffff"]], "hsv": { "hue": [0, 10] } } }),
        json!({ "collide": [{ "bounds": { "mask": { "texture": "nowhere" } } }] }),
        json!({ "collide": [{ "bounds": { "mask": { "texture": "song" } } }] }),
        json!({ "collide": [{ "bounds": { "mask": { "texture": "floor", "threshold": 2 } } }] }),
        json!({ "emitters": [{ "shape": { "mask": { "texture": "floor", "threshold": { "audio": "bass" } } }, "rate": 1 }] }),
        json!({ "render": { "trail": { "length": 99, "width": 0.5 } } }),
    ];
    for b in bad {
        assert!(!issues(b.clone()).0.is_empty(), "{b}");
    }
    let (_, long) = issues(json!({ "render": { "trail": { "length": 32, "width": 0.5 } } }));
    assert!(long.contains("particle_ropes"), "long trails stay with ropes");
}
