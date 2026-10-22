use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_scene::*;

const VIEW: [f32; 4] = [0.0, 0.0, 200.0, 100.0];

fn props() -> BTreeMap<String, Property> {
    serde_json::from_str(include_str!("props.json")).unwrap()
}

fn sparks() -> Json {
    json!({
        "capacity": 500, "seed": 3,
        "emitters": [{ "shape": { "box": [[0, 80], [200, 100]] }, "rate": 200 }],
        "init": { "life": [0.5, 1.5], "speed": [20, 60], "angle": [250, 290], "size": [2, 4] },
        "forces": [{ "gravity": [0, 30] }, { "turbulence": { "strength": 20, "scale": 0.05 } }, { "drag": 0.2 }],
        "collide": [{ "bounds": "canvas", "bounce": 0.5 }]
    })
}

fn with(mut base: Json, patch: Json) -> Json {
    patch.as_object().unwrap().iter().for_each(|(k, v)| base[k] = v.clone());
    base
}

fn scene(system: Json, visible: bool) -> String {
    let layer = json!({ "id": "p", "type": "particles", "system": "dots", "visible": visible });
    let assets = json!({ "dot": { "path": "dot.png", "atlas": [2, 2] } });
    json!({ "schema": SCHEMA, "canvas": { "size": [200, 100] }, "layers": [layer], "particles": { "dots": system }, "assets": assets })
        .to_string()
}

fn runtime(text: &str) -> Runtime {
    let props = props();
    Runtime::new(Arc::new(Scene::load(text, &props).unwrap()), effective(&props, &[]))
}

fn run(text: &str, frames: usize, quality: Quality) -> Cloud {
    let mut rt = runtime(text);
    let bake = Bake::new(&rt.scene, "dots", Tier::T3, false, |_| None).unwrap();
    let (mut cloud, mut stamp) = (Cloud::new(bake.alloc), 0);
    for _ in 0..frames {
        rt.advance(1.0 / 30.0);
        rt.frame(VIEW, quality);
        let swarm = rt.swarm(0).unwrap();
        if std::mem::replace(&mut stamp, swarm.stamp) != swarm.stamp {
            swarm.steps.iter().for_each(|s| cloud.step(&bake.apply(s), &bake.points));
        }
    }
    cloud
}

fn issues(system: Json) -> Vec<Issue> {
    Scene::from_json(&scene(system, true)).unwrap().validate(&props())
}

#[test]
fn systems_are_validated_against_their_limits() {
    let range =
        |path: &str, value: f32, min: f32, max: f32| Issue::Range { path: path.into(), value, min, max };
    let p = "/particles/dots";
    assert_eq!(issues(sparks()), []);
    for bad in [
        json!({ "forces": [{ "attractor": { "target": "mouse", "strength": 1 } }] }),
        json!({ "emitters": [{ "shape": "edge", "burst": { "count": 1, "on": "clap" } }] }),
        json!({ "render": { "frames": "sometimes" } }),
        json!({ "collide": [{ "bounds": "box" }] }),
        json!({ "init": { "glow": [1, 2] } }),
    ] {
        assert!(Scene::from_json(&scene(with(sparks(), bad.clone()), true)).is_err(), "{bad}");
    }
    assert_eq!(
        issues(with(sparks(), json!({ "capacity": 0 }))),
        [range(&format!("{p}/capacity"), 0.0, 1.0, 200_000.0)]
    );
    assert_eq!(
        issues(with(sparks(), json!({ "prewarm": 20 }))),
        [range(&format!("{p}/prewarm"), 20.0, 0.0, 10.0)]
    );
    let life = with(sparks(), json!({ "init": { "life": [3, 1] } }));
    assert_eq!(issues(life), [range(&format!("{p}/init/life/1"), 1.0, 3.0, 600.0)]);
    let five = Json::Array(vec![json!({ "shape": "edge", "rate": 1 }); 5]);
    let too_many = Issue::TooMany { path: format!("{p}/emitters"), count: 5, max: 4 };
    assert_eq!(issues(with(sparks(), json!({ "emitters": five }))), [too_many]);
    let bouncy = with(sparks(), json!({ "collide": [{ "bounds": "canvas", "bounce": 2 }] }));
    assert_eq!(issues(bouncy), [range(&format!("{p}/collide/0/bounce"), 2.0, 0.0, 1.0)]);
    let gone = Issue::Unknown { path: format!("{p}/render/texture"), what: "asset", id: "gone".into() };
    assert_eq!(issues(with(sparks(), json!({ "render": { "texture": "gone" } }))), [gone]);
    let rate = json!([{ "shape": "edge", "rate": { "prop": "nothing" } }]);
    let unknown =
        Issue::Unknown { path: format!("{p}/emitters/0/rate"), what: "property", id: "nothing".into() };
    assert_eq!(issues(with(sparks(), json!({ "emitters": rate }))), [unknown]);
    let curve = with(sparks(), json!({ "render": { "size_over_life": [[0.5, 1], [0.1, 0]] } }));
    assert_eq!(issues(curve), [Issue::Unsorted(format!("{p}/render/size_over_life"))]);
    let mut big = Scene::from_json(&scene(sparks(), true)).unwrap();
    let huge: System = serde_json::from_value(with(sparks(), json!({ "capacity": 150_000 }))).unwrap();
    big.particles.extend((0..8).map(|i| (format!("s{i}"), huge.clone())));
    let found = big.validate(&props());
    assert_eq!(found[0], Issue::TooMany { path: "/particles".into(), count: 9, max: 8 });
    assert!(matches!(&found[1], Issue::Range { path, value, .. } if path == "/particles" && *value > 1e6));
    let textured = with(
        sparks(),
        json!({ "render": { "texture": "dot", "frames": "life", "color_over_life": [[0, "#ffffff"], [1, "#ff000000"]], "trail": { "length": 8, "width": 0.5 } } }),
    );
    assert_eq!(issues(textured.clone()), []);
    let system: System = serde_json::from_value(textured).unwrap();
    let (sizes, colors) = system.render.luts();
    assert_eq!((sizes[0], sizes[63], colors[0], colors[63]), (1.0, 1.0, [1.0; 4], [1.0, 0.0, 0.0, 0.0]));
    assert!((colors[32][1] - (1.0 - 32.0 / 63.0)).abs() < 1e-5);
}

#[test]
fn the_cpu_simulation_is_deterministic_and_follows_quality() {
    let text = scene(sparks(), true);
    let a = run(&text, 120, Quality::Q3);
    assert_eq!(a.particles, run(&text, 120, Quality::Q3).particles);
    let census = Census::of(&a.particles);
    assert!(census.alive > 100 && census.alive == a.alive(), "{census:?}");
    assert!(census.min[0] >= 0.0 && census.max[0] <= 200.0 && census.min[1] >= 0.0 && census.max[1] <= 100.0);
    assert!(census.mean[1] < 90.0, "particles rise: {census:?}");
    assert_ne!(a.particles, run(&text.replace("\"seed\":3", "\"seed\":4"), 120, Quality::Q3).particles);
    let low = Census::of(&run(&text, 120, Quality::Q0).particles);
    assert!(low.alive <= 125 && low.alive * 3 < census.alive, "{low:?}");
}

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
    let cued = json!({ "capacity": 50, "emitters": [
        { "shape": { "point": [10, 10] }, "burst": { "count": 7, "on": "beat" } },
        { "shape": { "point": [90, 10] }, "burst": { "count": 3, "on": "click" }, "follow": "cursor" }
    ] });
    let mut rt = runtime(&scene(cued, true));
    assert_eq!((rt.uses().audio, rt.uses().cursor), (true, true));
    rt.frame(VIEW, Quality::Q3);
    assert_eq!(rt.swarm(0).unwrap().steps[0].births[3], 0);
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
    assert_eq!((step.births[0], step.births[1], step.emitters[3]), (7, 10, [100.0, 50.0, 0.0, 0.0]));
}

#[test]
fn budgets_follow_the_device_tier_and_masks_place_particles() {
    let big = with(sparks(), json!({ "capacity": 8000, "init": { "size": [40, 60] } }));
    let s = Scene::from_json(&scene(big, true)).unwrap();
    let bake = |tier, cpu| Bake::new(&s, "dots", tier, cpu, |_| None).unwrap();
    assert_eq!(
        [bake(Tier::T0, true).alloc, bake(Tier::T2, true).alloc, bake(Tier::T2, false).alloc],
        [2000, 5000, 8000]
    );
    let (t0, t3) = (bake(Tier::T0, true), bake(Tier::T3, false));
    assert!(t0.size < t3.size && t3.size < 1.0, "{} {}", t0.size, t3.size);
    assert!(8000.0 * (50.0 * t3.size).powi(2) <= 6.0 * 200.0 * 100.0 * 1.01);
    assert_eq!((t0.trail, t3.trail, Quality::Q2.trail(8), Quality::Q1.trail(8)), (0, 16, 4, 0));
    let masked = with(
        sparks(),
        json!({ "emitters": [{ "shape": { "mask": { "texture": "dot" } }, "burst": { "count": 300 } }],
        "forces": [], "collide": [], "init": { "speed": [0, 0], "life": [5, 5] } }),
    );
    let rt = runtime(&scene(masked, true));
    let left: Vec<u8> = (0..64).flat_map(|i| [255, 255, 255, if i % 8 < 4 { 255 } else { 0 }]).collect();
    let bake =
        Bake::new(&rt.scene, "dots", Tier::T3, true, |id| (id == "dot").then_some((8, 8, left.as_slice())))
            .unwrap();
    rt.frame(VIEW, Quality::Q3);
    let mut cloud = Cloud::new(bake.alloc);
    rt.swarm(0).unwrap().steps.iter().for_each(|s| cloud.step(&bake.apply(s), &bake.points));
    let census = Census::of(&cloud.particles);
    assert!(census.alive == 300 && census.max[0] <= 100.0 && census.min[0] >= 0.0, "{census:?}");
}

#[test]
fn timed_bursts_keep_their_beat_while_the_scene_sleeps() {
    let every = json!({ "capacity": 10, "emitters": [{ "shape": "edge", "burst": { "count": 5, "every": 2.0 } }],
        "init": { "life": [0.2, 0.2] } });
    let mut rt = runtime(&scene(every, true));
    let (mut wake, mut t, mut frames, mut bursts) = (rt.frame(VIEW, Quality::Q3).wake, 0.0, 0, vec![0.0]);
    let mut stamp = rt.swarm(0).unwrap().stamp;
    while t < 9.0 {
        let gap = match wake {
            Wake::Frame => 1.0 / 30.0,
            Wake::At(s) => s.max(1.0 / 30.0),
            Wake::Idle => break,
        };
        (t, frames) = (t + gap, frames + 1);
        rt.advance(gap);
        wake = rt.frame(VIEW, Quality::Q3).wake;
        let swarm = rt.swarm(0).unwrap();
        if std::mem::replace(&mut stamp, swarm.stamp) != swarm.stamp
            && swarm.steps.iter().any(|s| s.births[3] > 0)
        {
            bursts.push(t);
        }
    }
    let gaps: Vec<f64> = bursts.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(gaps.len() == 4 && gaps.iter().all(|g| (g - 2.0).abs() < 0.05), "{bursts:?}");
    assert!(frames < 50, "the scene sleeps between bursts, {frames} frames");
}

#[test]
fn particles_keep_real_time_at_low_frame_rates() {
    for fps in [60.0, 24.0, 10.0, 5.0, 1.0] {
        let mut rt = runtime(&scene(sparks(), true));
        rt.frame(VIEW, Quality::Q3);
        let (mut simulated, mut stamp) = (0.0, rt.swarm(0).unwrap().stamp);
        for _ in 0..(4.0 * fps) as usize {
            rt.advance(1.0 / fps);
            rt.frame(VIEW, Quality::Q3);
            let swarm = rt.swarm(0).unwrap();
            if std::mem::replace(&mut stamp, swarm.stamp) != swarm.stamp {
                simulated += swarm.steps.iter().map(|s| f64::from(s.clock[0])).sum::<f64>();
            }
        }
        assert!((simulated - 4.0).abs() < 0.02, "{fps} fps simulates {simulated} s in 4 s");
    }
}

#[test]
fn extreme_systems_neither_overflow_nor_panic() {
    let flood = json!({ "capacity": 300, "emitters": [
            { "shape": "edge", "rate": { "keys": [[0, 1e6], [1, 1e6]], "scale": 1e6 }, "burst": { "count": 200000, "every": 0.02 } },
            { "shape": "edge", "rate": 1e5, "burst": { "count": 200000, "on": "beat" } }],
        "init": { "life": [200, 200], "speed": [0, 0] },
        "forces": [{ "gravity": [1e6, 0] }, { "turbulence": { "strength": 10, "scale": 1 } }] });
    let text = scene(flood, true);
    assert_eq!(issues(serde_json::from_str::<Json>(&text).unwrap()["particles"]["dots"].clone()), []);
    let cloud = run(&text, 2400, Quality::Q3);
    assert_eq!(cloud.alive(), 300);
    let mut rt = runtime(&text);
    for _ in 0..3 {
        rt.advance(0.5);
        rt.frame(VIEW, Quality::Q3);
        assert!(rt.swarm(0).unwrap().steps.iter().all(|s| s.births[0] <= s.births[3] && s.births[3] <= 300));
    }
}

#[test]
fn layers_drawing_one_system_share_its_budget() {
    let big = with(sparks(), json!({ "capacity": 1500, "init": { "size": [40, 60] } }));
    let mut s = Scene::from_json(&scene(big, true)).unwrap();
    let one = Bake::new(&s, "dots", Tier::T0, true, |_| None).unwrap();
    let layer = s.layers[0].clone();
    s.layers.extend((1..8).map(|i| Layer { id: format!("p{i}"), ..layer.clone() }));
    let eight = Bake::new(&s, "dots", Tier::T0, true, |_| None).unwrap();
    assert_eq!((one.alloc, eight.alloc), (1500, 250));
    assert!(eight.size < one.size, "{} {}", eight.size, one.size);
}
