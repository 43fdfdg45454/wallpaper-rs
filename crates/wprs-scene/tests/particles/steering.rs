use super::*;

fn steered(forces: Json, life: f32) -> Json {
    with(
        sparks(),
        json!({ "emitters": [{ "shape": { "box": [[20, 20], [180, 80]] }, "rate": 200 }],
            "init": { "life": [life, life], "speed": [0, 30], "angle": [0, 360], "size": [4, 4] },
            "forces": forces, "collide": [], "points": [{ "at": [100, 50] }] }),
    )
}

fn alive(system: Json, frames: usize) -> Vec<Particle> {
    run(&scene(system, true), frames, Quality::Q3).particles.into_iter().filter(|p| p.age < p.life).collect()
}

#[test]
fn keep_distance_settles_particles_on_a_ring_around_the_target() {
    let forces =
        json!([{ "keep_distance": { "target": "p0", "distance": 30, "strength": 20 } }, { "drag": 4 }]);
    let all = alive(steered(forces, 30.0), 240);
    let old: Vec<&Particle> = all.iter().filter(|p| p.age > 3.0).collect();
    assert!(old.len() > 20, "{}", old.len());
    for p in old {
        let d = (p.pos[0] - 100.0).hypot(p.pos[1] - 50.0);
        assert!((d - 30.0).abs() < 2.0, "{d} {p:?}");
    }
}

#[test]
fn keep_between_draws_particles_onto_the_segment() {
    let forces =
        json!([{ "keep_between": { "from": [40, 30], "to": [160, 70], "strength": 25 } }, { "drag": 5 }]);
    let all = alive(steered(forces, 30.0), 240);
    let line =
        |p: &Particle| ((p.pos[0] - 40.0) * 40.0 - (p.pos[1] - 30.0) * 120.0).abs() / 120f32.hypot(40.0);
    let old: Vec<&Particle> = all.iter().filter(|p| p.age > 3.0).collect();
    assert!(!old.is_empty() && old.iter().all(|p| line(p) < 1.0), "{old:?}");
    assert!(old.iter().all(|p| (39.0..=161.0).contains(&p.pos[0])));
}

#[test]
fn remap_operators_follow_the_age_of_each_particle() {
    let set = json!({ "remap": { "input": "age", "in": [0, 2], "clamp_in": true, "output": "size", "out": [2, 12] } });
    let all = alive(steered(json!([set]), 3.0), 60);
    assert!(!all.is_empty());
    for p in all.iter().filter(|p| p.age > 0.0) {
        let want = 2.0 + 10.0 * (p.age / 2.0).min(1.0);
        assert!((p.size - want).abs() < 0.2, "{} {p:?}", want);
    }
    let grow = json!({ "remap": { "input": "random", "in": [0, 1], "output": "alpha", "out": [0.5, 0.5], "op": "sub" } });
    let faded = alive(steered(json!([grow]), 3.0), 30);
    assert!(faded.iter().all(|p| (p.color[3] - (1.0 - 0.5 * p.age).max(0.0)).abs() < 0.05), "{faded:?}");
}

#[test]
fn boids_align_their_flock_and_keep_apart() {
    let flock = json!({ "boids": { "radius": 40, "separation": 2, "alignment": 3, "cohesion": 0.5, "max_speed": 50 } });
    let mut s = steered(json!([flock]), 30.0);
    s["emitters"][0] = json!({ "shape": { "box": [[60, 30], [140, 70]] }, "burst": { "count": 40 } });
    let all = alive(s, 180);
    assert_eq!(all.len(), 40);
    assert!(all.iter().all(|p| p.vel[0].hypot(p.vel[1]) <= 50.01), "{all:?}");
    let mean = all.iter().fold([0.0f32; 2], |m, p| [m[0] + p.vel[0] / 40.0, m[1] + p.vel[1] / 40.0]);
    let spread: f32 = all.iter().map(|p| (p.vel[0] - mean[0]).hypot(p.vel[1] - mean[1])).sum::<f32>() / 40.0;
    assert!(spread < 10.0, "{spread} {mean:?}");
}

#[test]
fn fewer_neighbours_steer_boids_at_lower_quality() {
    assert_eq!(
        [Quality::Q0, Quality::Q1, Quality::Q2, Quality::Q3].map(Quality::neighbours),
        [8, 16, 32, 32]
    );
    assert_eq!([Tier::T0, Tier::T1, Tier::T2, Tier::T3].map(Tier::neighbours), [8, 16, 32, 32]);
    let flock =
        json!({ "boids": { "radius": 30, "separation": 1, "alignment": 1, "cohesion": 1, "max_speed": 0 } });
    let text = scene(steered(json!([flock]), 5.0), true);
    let mut rt = runtime(&text);
    let bake = Bake::new(&rt.scene, 0, Tier::T1, true, |_| None).unwrap();
    for q in [Quality::Q3, Quality::Q0] {
        rt.advance(1.0 / 30.0);
        rt.frame(VIEW, q);
        let sim = bake.apply(0, rt.swarm(0).unwrap().sim(0, 0));
        assert_eq!(sim.flock[2], q.neighbours().min(16), "{q:?}");
        assert_eq!(sim.flock[1], cells(bake.allocs[0]));
    }
    let big = with(steered(json!([flock]), 5.0), json!({ "capacity": 8000 }));
    let rt = runtime(&scene(big, true));
    assert_eq!(Bake::new(&rt.scene, 0, Tier::T3, true, |_| None).unwrap().allocs[0], CPU_BOIDS);
}
