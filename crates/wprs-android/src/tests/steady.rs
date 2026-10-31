use super::{ask, renderer};
use crate::stage::Stage;
use crate::stage::report::wall;
use crate::{counted, pack, power};
use serde_json::json;
use std::path::Path;
use std::time::{Duration, Instant};

fn wall_clock() {
    let local = chrono::Local::now().naive_local().and_utc().timestamp_millis() as f64 / 1e3;
    assert!((wall() - local).abs() < 1.0, "the scene clock is {} and the local time {local}", wall());
    let start = Instant::now();
    counted::watch(true);
    while start.elapsed() < Duration::from_millis(1100) {
        std::hint::black_box(wall());
    }
    let (made, _) = counted::watch(false);
    assert_eq!(made, 0, "reading the local time for each frame allocated {made} times in a second");
}

pub(super) fn frames() {
    wall_clock();
    let Some(r) = renderer() else { return };
    let power =
        power::Power { charging: true, saver: false, low: false, thermal: 0, headroom: 0.0, still: false };
    let listed = ask(json!({ "op": "items", "query": "kind:scene" }));
    for item in listed.as_array().unwrap() {
        let (uid, path) = (item["uid"].as_str().unwrap(), item["path"].as_str().unwrap());
        let mut stage = Stage::new(uid.into(), pack::load(Path::new(path)).unwrap());
        let mut made = Vec::new();
        for n in 0..90 {
            counted::watch(true);
            let nanos = n * 1_000_000_000 / 60;
            stage.render(r, ((64, 128), nanos, 60), (power, wprs_scene::Tier::T1), &[], true).unwrap();
            made.push(counted::watch(false).0);
        }
        let steady: u64 = made[30..].iter().sum();
        let scripted = if stage.pack.scripts.is_empty() { 0 } else { STEPS };
        let title = item["title"].as_str().unwrap();
        eprintln!("{title}: {} allocations in 60 steady frames besides the GPU calls", steady);
        assert!(
            steady <= 60 * scripted,
            "{title} made {steady} allocations in 60 frames besides the GPU calls"
        );
    }
}

const STEPS: u64 = 16;
