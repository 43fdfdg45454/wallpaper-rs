use super::*;

fn driven(forces: Json, init: Json) -> Fly {
    let mut system = still(json!({ "forces": forces }));
    init.as_object().unwrap().iter().for_each(|(k, v)| system["init"][k] = v.clone());
    Fly::of(json!({ "a": system }))
}

fn speed(p: &Particle) -> f32 {
    p.vel[0].hypot(p.vel[1])
}

#[test]
fn caps_spins_windows_and_brakes() {
    let mut capped = driven(json!([{ "gravity": [0, 4000] }, { "cap": 25 }]), json!({}));
    capped.run(0.5);
    assert!(capped.alive(0).iter().all(|p| speed(p) <= 25.01), "{:?}", capped.alive(0)[0].vel);
    let mut spun = driven(json!([{ "torque": 90 }, { "spin_drag": 1 }]), json!({}));
    spun.run(1.0);
    let spin = spun.alive(0)[0].spin;
    assert!(spin > 40.0 && spin < 90.0, "{spin}");
    let late = json!([{ "gravity": [0, 100], "window": [0.5, 0.5, 1, 1] }]);
    let mut waited = driven(late.clone(), json!({ "life": [2, 2] }));
    waited.run(0.9);
    assert!(waited.alive(0).iter().all(|p| p.vel == [0.0, 0.0]), "no gravity before half the life");
    waited.run(0.3);
    assert!(waited.alive(0).iter().all(|p| p.vel[1] > 5.0));
    let brake = json!([{ "slow_near": { "target": [100, 50], "inner": 5, "outer": 10, "factor": 0 } }]);
    let mut braked = driven(brake, json!({ "speed": [30, 30] }));
    braked.run(0.2);
    assert!(braked.alive(0).iter().all(|p| speed(p) < 1e-3 && near(p.pos, [100.0, 50.0], 5.0)));
}

#[test]
fn pulls_reach_only_as_far_as_their_radius() {
    let near_pull = json!([{ "attractor": { "target": [130, 50], "strength": 100, "radius": 20 } }]);
    let mut far = driven(near_pull, json!({}));
    far.run(0.3);
    assert!(far.alive(0).iter().all(|p| p.vel == [0.0, 0.0]), "30 px is beyond the radius");
    let swirl = json!([{ "vortex": { "center": [100, 60], "strength": 50, "falloff": "constant", "ring": { "radius": 20, "width": 5, "pull": 200 } } }]);
    let mut swirled = driven(swirl, json!({}));
    swirled.run(0.5);
    let spread = swirled
        .alive(0)
        .iter()
        .map(|p| ((p.pos[0] - 100.0).hypot(p.pos[1] - 60.0) - 20.0).abs())
        .fold(0.0, f32::max);
    assert!(spread < 9.0, "the ring pulls particles to its radius: {spread}");
}

#[test]
fn oscillation_wobbles_without_drifting() {
    let wobble = json!([{ "oscillate": { "what": "position", "freq": [1, 1], "scale": [5, 5], "mask": [1, 0] } },
        { "oscillate": { "what": "alpha", "freq": [2, 2], "scale": [1, 1] } },
        { "oscillate": { "what": "size", "freq": [2, 2], "scale": [0.5, 0.5] } }]);
    let mut fly = driven(wobble, json!({}));
    let (mut xs, mut alphas, mut sizes) = (Vec::new(), Vec::new(), Vec::new());
    for _ in 0..60 {
        fly.frame(1.0 / 30.0);
        let p = fly.alive(0)[0];
        xs.push(p.pos[0]);
        alphas.push(p.look[0]);
        sizes.push(p.look[1]);
    }
    let range =
        |v: &[f32]| v.iter().copied().fold(f32::MIN, f32::max) - v.iter().copied().fold(f32::MAX, f32::min);
    assert!(range(&xs) > 6.0 && range(&xs) < 10.01, "{xs:?}");
    assert!((xs[59] - xs[29]).abs() < 0.5, "a whole cycle comes back: {} {}", xs[29], xs[59]);
    assert!(alphas.iter().all(|a| (0.0..=1.0).contains(a)) && range(&alphas) > 0.5);
    assert!(sizes.iter().all(|s| (0.5..=1.5).contains(s)) && range(&sizes) > 0.5);
    assert!(fly.alive(0).iter().all(|p| p.pos[1] == 50.0), "the mask keeps y still");
}
