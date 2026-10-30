use serde_json::json;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};
use wprs_scene::*;

mod cached;
mod failed;

use cached::*;
use failed::*;

const GOOD: &str = "@fragment fn fs_h(in: VertexOut) -> @location(0) vec4<f32> { return tap(in.uv); }";
const SLOW: &str = "@fragment fn fs_h(in: VertexOut) -> @location(0) vec4<f32> { return tap(in.uv).gbra; }";
const ENDLESS: &str = "@fragment fn fs_h(in: VertexOut) -> @location(0) vec4<f32> {
    var d = tap(in.uv);
    loop { d *= 0.5; if d.a < -1.0 { break; } }
    return d;
}";

fn probe(id: &str) {
    match id {
        "abort" => std::process::abort(),
        "huge" => drop(std::hint::black_box(vec![1u8; 2 << 30])),
        "hang" => std::thread::sleep(Duration::from_secs(60)),
        "spin" => loop {
            std::hint::black_box(0u64);
        },
        _ => {}
    }
}

fn json(id: &str) -> String {
    json!({ "id": id, "cost": {}, "passes": [{ "entry": "fs_h" }] }).to_string()
}

fn lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path).unwrap_or_default().lines().map(String::from).collect()
}

type Change = fn(&mut Checker);

fn checker(change: impl FnOnce(&mut Checker)) {
    let mut checker = Checker::default();
    change(&mut checker);
    set_checker(Some(checker));
}

fn timed<T>(work: impl FnOnce() -> T) -> (T, Duration) {
    let start = Instant::now();
    (work(), start.elapsed())
}

fn main() {
    serve_check_with(probe);
    let args: Vec<String> = std::env::args().collect();
    match args.as_slice() {
        [_, flag, book] if flag == "--again" => return again(book),
        [_, flag, book] if flag == "--limited" => return limited(book),
        [_, flag, book, script] if flag == "--slow" => return slow(book, script),
        _ => {}
    }
    let dir = tempfile::tempdir().unwrap();
    let book = dir.path().join("cache").join("stopped-effects");
    keep_strikes(&book);
    checker(|_| ());
    accepted_shaders_run_the_helper_once(&book);
    refused_shaders_return_the_message_of_the_helper(&book);
    failed_helpers_refuse_without_strikes(&book);
    the_disk_cache_survives_a_new_process(&book);
    lower_hard_limits_of_the_parent_still_let_the_helper_check(&book);
    a_process_killed_while_the_helper_runs_gets_no_strike(&book);
    set_checker(None);
    println!("test result: ok. 6 passed");
}
