use super::*;

fn flowing(more: Json) -> Json {
    let mut e = json!({ "shape": { "point": [100, 50] }, "rate": 60 });
    more.as_object().unwrap().iter().for_each(|(k, v)| e[k] = v.clone());
    still(json!({ "capacity": 2000, "emitters": [e], "init": { "life": [0.1, 0.1], "speed": [0, 0] } }))
}

#[test]
fn emitters_wait_stop_and_sleep() {
    let mut fly = Fly::of(json!({ "a": flowing(json!({ "delay": 1, "duration": 0.5 })) }));
    let wake = fly.frame(0.0);
    assert!(matches!(wake, Wake::At(t) if (t - 1.0).abs() < 0.02), "{wake:?}");
    fly.run(0.9);
    assert_eq!(fly.births, 0);
    fly.run(0.3);
    assert!(fly.births > 0);
    fly.run(1.0);
    let (births, wake) = (fly.births, fly.frame(1.0 / 30.0));
    assert!((25..=35).contains(&births) && wake == Wake::Idle, "{births} {wake:?}");
}

#[test]
fn periodic_windows_and_single_births_per_step() {
    let mut fly =
        Fly::of(json!({ "a": flowing(json!({ "periodic": { "on": [0.5, 0.5], "off": [0.5, 0.5] } })) }));
    let mut counts = Vec::new();
    for _ in 0..4 {
        let before = fly.births;
        fly.run(0.5);
        counts.push(fly.births - before);
    }
    assert!(counts[0] > 20 && counts[1] < 3 && counts[2] > 20 && counts[3] < 3, "{counts:?}");
    let mut one = Fly::of(json!({ "a": flowing(json!({ "rate": 600, "max_per_step": 1 })) }));
    one.run(1.0);
    let swarm = one.rt.swarm(0).unwrap();
    assert!((0..swarm.count()).all(|j| swarm.sim(j, 0).total() <= 1));
    assert!((55..=65).contains(&one.births), "{}", one.births);
}

#[test]
fn gated_emitters_wait_for_sound() {
    let mut fly = Fly::of(json!({ "a": flowing(json!({ "audio_gate": true })) }));
    fly.rt.set_audio(AudioFrame { silent: true, ..AudioFrame::default() });
    fly.run(0.5);
    assert_eq!(fly.births, 0);
    fly.rt.set_audio(AudioFrame { silent: false, level: 0.5, ..AudioFrame::default() });
    fly.run(0.5);
    assert!(fly.births > 20, "{}", fly.births);
    assert!(fly.rt.uses().audio);
}

#[test]
fn prewarm_runs_in_coarse_steps() {
    let warm = still(json!({ "prewarm": 30, "emitters": [{ "shape": { "point": [100, 50] }, "rate": 10 }] }));
    let mut fly = Fly::of(json!({ "a": warm }));
    fly.frame(0.0);
    let swarm = fly.rt.swarm(0).unwrap();
    assert_eq!((swarm.count(), swarm.sim(0, 0).clock[0]), (450, WARM));
    let big = still(json!({ "capacity": 100000, "prewarm": 30 }));
    let layers = json!([{ "id": "p", "type": "particles", "system": "a" }, { "id": "q", "type": "particles", "system": "b" }]);
    let rt = runtime(json!({ "a": big.clone(), "b": big }), layers);
    rt.frame(VIEW, Quality::Q3);
    let steps: Vec<usize> = (0..2).map(|i| rt.swarm(i).unwrap().count()).collect();
    assert_eq!(steps, [150, 150], "the scene shares one warm-up budget");
}
