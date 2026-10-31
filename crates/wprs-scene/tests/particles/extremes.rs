use super::*;

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
            && swarm.steps.iter().any(|s| s.total() > 0)
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
        assert!(rt.swarm(0).unwrap().steps.iter().all(|s| s.births[0] <= s.total() && s.total() <= 300));
    }
}

#[test]
fn layers_drawing_one_system_share_its_budget() {
    let big = with(sparks(), json!({ "capacity": 1500, "init": { "size": [40, 60] } }));
    let mut s = Scene::from_json(&scene(big, true)).unwrap();
    let one = Bake::new(&s, 0, Tier::T0, true, |_| None).unwrap();
    let layer = s.layers[0].clone();
    s.layers.extend((1..8).map(|i| Layer { id: format!("p{i}"), ..layer.clone() }));
    let eight = Bake::new(&s, 0, Tier::T0, true, |_| None).unwrap();
    assert_eq!((one.allocs[0], eight.allocs[0]), (1500, 250));
    assert!(eight.sizes[0] < one.sizes[0], "{:?} {:?}", eight.sizes, one.sizes);
}

#[test]
fn ranges_of_births_and_emitter_shapes_may_be_bound() {
    let loud = json!({ "audio": "bass", "scale": 8.0 });
    let circle = json!({ "circle": { "center": [100, 50], "radius": loud, "inner": 2 } });
    let bound = with(sparks(), json!({ "emitters": [{ "shape": circle, "rate": 10 }] }));
    let mut init = bound["init"].clone();
    init["size"] = json!([2, loud]);
    init["life"] = json!([0.5, { "keys": [[0, 1], [2, 2]] }, 2]);
    let text = scene(with(bound, json!({ "init": init })), true);
    let parsed = Scene::from_json(&text).unwrap();
    let found = parsed.validate(&props());
    assert!(found.is_empty(), "{found:?}");
    assert!(parsed.features().contains("particle_values") && needs(&parsed).audio);
    let system = &parsed.particles["dots"];
    assert_eq!((system.init.size.ends, system.init.life.exponent), ([2.0, 0.0], Some(2.0)));
    let written: Json = serde_json::to_value(&parsed).unwrap();
    assert_eq!(written["particles"]["dots"]["init"]["size"], json!([2.0, loud]));
    assert_eq!(written["particles"]["dots"]["emitters"][0]["shape"]["circle"]["radius"], loud);
    let plain = Scene::from_json(&scene(sparks(), true)).unwrap();
    assert!(!plain.features().contains("particle_values") && !needs(&plain).audio);
    let colored = with(sparks(), json!({ "init": { "color": [{ "audio": "bass" }, "#ffffff"] } }));
    assert!(matches!(&issues(colored)[..], [Issue::Type(p)] if p.ends_with("/init/color")));
}
