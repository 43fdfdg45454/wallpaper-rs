use super::*;

#[test]
fn paused_and_hidden_systems_do_no_work() {
    let mut rt = runtime(&scene(sparks(), true));
    assert_eq!(rt.frame(VIEW, Quality::Q3).wake, Wake::Frame);
    let first = rt.swarm(0).unwrap().stamp;
    assert_eq!(rt.swarm(0).unwrap().steps.len(), 1);
    for _ in 0..5 {
        assert_eq!(rt.frame(VIEW, Quality::Q3).wake, Wake::Frame);
    }
    assert_eq!(rt.swarm(0).unwrap().stamp, first, "no time passed, so nothing is simulated");
    rt.advance(1.0 / 30.0);
    rt.frame(VIEW, Quality::Q3);
    assert_eq!((rt.swarm(0).unwrap().stamp, rt.swarm(0).unwrap().steps.len()), (first + 1, 2));
    rt.advance(10.0);
    rt.frame(VIEW, Quality::Q3);
    assert_eq!(rt.swarm(0).unwrap().steps.len(), SUBSTEPS, "a pause is never caught up");
    let epoch = rt.swarm(0).unwrap().epoch;
    rt.seek(0.5);
    assert_eq!(rt.swarm(0).unwrap().epoch, epoch + 1);
    let mut hidden = runtime(&scene(sparks(), false));
    hidden.advance(1.0);
    assert_eq!((hidden.frame(VIEW, Quality::Q3).wake, hidden.swarm(0).unwrap().stamp), (Wake::Idle, 0));
}

#[test]
fn the_scheduler_knows_when_particles_live() {
    let fading = json!({ "capacity": 100, "emitters": [{ "shape": { "point": [100, 50] }, "rate": { "prop": "density" } }],
        "init": { "life": [0.5, 1.0] } });
    let mut rt = runtime(&scene(fading, true));
    assert_eq!(rt.frame(VIEW, Quality::Q3).wake, Wake::Frame);
    let mut values = rt.props().clone();
    values.insert("density".into(), json!(0));
    rt.set_props(values);
    let mut wakes = Vec::new();
    for _ in 0..30 {
        rt.advance(1.0 / 15.0);
        wakes.push(rt.frame(VIEW, Quality::Q3).wake);
    }
    assert_eq!((wakes[0], wakes[29]), (Wake::Frame, Wake::Idle));
    let every = json!({ "capacity": 10, "emitters": [{ "shape": "edge", "burst": { "count": 5, "every": 2.0 } }],
        "init": { "life": [0.2, 0.2] } });
    let mut rt = runtime(&scene(every, true));
    assert_eq!(rt.frame(VIEW, Quality::Q3).wake, Wake::Frame);
    for _ in 0..5 {
        rt.advance(1.0 / 15.0);
        rt.frame(VIEW, Quality::Q3);
    }
    assert!(
        matches!(rt.frame(VIEW, Quality::Q3).wake, Wake::At(t) if (t - 1.65).abs() < 0.05),
        "{:?}",
        rt.wake()
    );
    let long = json!({ "prop": "depth", "scale": 25 });
    let shrinking = json!({ "capacity": 10, "emitters": [{ "shape": { "point": [10, 10] }, "burst": { "count": 10 } }],
        "init": { "life": [long, long] } });
    let mut rt = runtime(&scene(shrinking, true));
    rt.frame(VIEW, Quality::Q3);
    let mut values = rt.props().clone();
    values.insert("depth".into(), json!(0.02));
    rt.set_props(values);
    let mut wake = || {
        rt.advance(1.0 / 15.0);
        rt.frame(VIEW, Quality::Q3).wake
    };
    let wakes: Vec<Wake> = (0..160).map(|_| wake()).collect();
    assert!(wakes[..148].iter().all(|w| *w == Wake::Frame) && wakes[152..].iter().all(|w| *w == Wake::Idle));
    let cued = json!({ "capacity": 50, "emitters": [
        { "shape": { "point": [10, 10] }, "burst": { "count": 7, "on": "beat" } },
        { "shape": { "point": [90, 10] }, "burst": { "count": 3, "on": "click" }, "follow": "cursor" }
    ] });
    let mut rt = runtime(&scene(cued, true));
    assert_eq!((rt.uses().audio, rt.uses().cursor), (true, true));
    rt.frame(VIEW, Quality::Q3);
    assert_eq!(rt.swarm(0).unwrap().steps[0].total(), 0);
    assert!(rt.set_audio(AudioFrame { beat: true, silent: false, ..AudioFrame::default() }));
    rt.set_pointer(PointerState {
        pos: Some([0.5, 0.5]),
        smooth: [0.5, 0.5],
        buttons: 1,
        inside: true,
        ..PointerState::default()
    });
    rt.advance(1.0 / 60.0);
    rt.frame(VIEW, Quality::Q3);
    let step = rt.swarm(0).unwrap().steps[0];
    assert_eq!((step.births[0], step.births[1], step.emitters[7]), (7, 10, [100.0, 50.0, 0.0, 0.0]));
}

#[test]
fn budgets_follow_the_device_tier_and_masks_place_particles() {
    let big = with(sparks(), json!({ "capacity": 8000, "init": { "size": [40, 60] } }));
    let s = Scene::from_json(&scene(big, true)).unwrap();
    let bake = |tier, cpu| Bake::new(&s, 0, tier, cpu, |_| None).unwrap();
    assert_eq!(
        [bake(Tier::T0, true).allocs[0], bake(Tier::T2, true).allocs[0], bake(Tier::T2, false).allocs[0]],
        [2000, 5000, 8000]
    );
    let (t0, t3) = (bake(Tier::T0, true), bake(Tier::T3, false));
    let (small, large) = (t0.sizes[0], t3.sizes[0]);
    assert!(small < large && large < 1.0, "{small} {large}");
    assert!(8000.0 * (50.0 * large).powi(2) <= 6.0 * 200.0 * 100.0 * 1.01);
    assert_eq!(
        (Tier::T0.trail(), Tier::T3.trail(), Quality::Q2.trail(8), Quality::Q1.trail(8)),
        (0, 16, 4, 0)
    );
    assert_eq!((t0.trails[0], t3.trails[0]), (0, 0), "no trail is kept for a render without one");
    let masked = with(
        sparks(),
        json!({ "emitters": [{ "shape": { "mask": { "texture": "dot" } }, "burst": { "count": 300 } }],
        "forces": [], "collide": [], "init": { "speed": [0, 0], "life": [5, 5] } }),
    );
    let rt = runtime(&scene(masked, true));
    let left: Vec<u8> = (0..64).flat_map(|i| [255, 255, 255, if i % 8 < 4 { 255 } else { 0 }]).collect();
    let bake = Bake::new(&rt.scene, 0, Tier::T3, true, |id| (id == "dot").then_some((8, 8, left.as_slice())))
        .unwrap();
    rt.frame(VIEW, Quality::Q3);
    let mut cloud = Cloud::new(bake.allocs[0]);
    rt.swarm(0).unwrap().steps.iter().for_each(|s| cloud.step(&bake.apply(0, s), &bake.points));
    let census = Census::of(&cloud.particles);
    assert!(census.alive == 300 && census.max[0] <= 100.0 && census.min[0] >= 0.0, "{census:?}");
}
