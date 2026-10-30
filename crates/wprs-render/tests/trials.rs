mod hostile;

use hostile::{entry, frame, renderer, scene, show, unchecked};
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};
use wprs_render::{drawn_effects, set_build_timeout};

static ONE: Mutex<()> = Mutex::new(());

fn book(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    drop(std::fs::remove_dir_all(&dir));
    let path = dir.join("stopped-effects");
    wprs_scene::keep_strikes(&path);
    path
}

fn trials(path: &PathBuf, key: u64) -> usize {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    text.lines().filter(|l| l.starts_with(&format!("{key:016x} trying "))).count()
}

fn slow() -> String {
    let mut s = String::from("fn f0(m: mat4x4<f32>) -> mat4x4<f32> { var a = m; let n = m * 0.999; ");
    s.push_str(&"a = a * n;".repeat(200));
    s.push_str(" return a; }\nfn f1(m: mat4x4<f32>) -> mat4x4<f32> { var a = m; ");
    s.push_str(&"a = f0(a);".repeat(8));
    let salt = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos();
    let body = format!("let v = tap(in.uv) * f32({salt}u); return f1(mat4x4<f32>(v, v, v, v))[0];");
    s + " return a; }\n" + &entry(&body)
}

#[test]
fn shaders_that_build_too_long_are_left_out_while_the_rest_of_the_scene_draws() {
    let _one = ONE.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(r) = renderer() else { return };
    let path = book("slow");
    set_build_timeout(Duration::from_millis(300));
    let mut s = scene(&[("slow", entry("return tap(in.uv);")), ("fine", entry("return tap(in.uv) * 0.5;"))]);
    let key = unchecked(&mut s, "slow", &slow());
    let started = Instant::now();
    let mut shown = show(r, s).unwrap();
    set_build_timeout(Duration::from_secs(10));
    let left = shown.view.left_out();
    assert!(started.elapsed() < Duration::from_secs(5), "{:?}", started.elapsed());
    assert!(left.len() == 1 && left[0].contains("slow") && left[0].contains("to build"), "{left:?}");
    frame(r, &mut shown).unwrap();
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(text.contains(&format!("{key:016x} 1 ")) && !wprs_scene::struck(key), "{text}");
    while drawn_effects().contains(&key) && started.elapsed() < Duration::from_secs(300) {
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn only_the_effects_of_frames_that_did_not_finish_are_blamed_for_a_lost_device() {
    let _one = ONE.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(r) = renderer() else { return };
    book("views");
    let (left, right) = (
        scene(&[("left", entry("return tap(in.uv).gbra;"))]),
        scene(&[("right", entry("return tap(in.uv).brga;"))]),
    );
    let b = right.loaded["right"].key;
    let (mut left, mut right) = (show(r, left).unwrap(), show(r, right).unwrap());
    frame(r, &mut right).unwrap();
    r.finish().unwrap();
    frame(r, &mut left).unwrap();
    let drawn = drawn_effects();
    assert!(!drawn.contains(&b), "{drawn:?} {b}");
}

#[test]
fn new_package_effects_stay_on_trial_until_their_first_frames_finish() {
    let _one = ONE.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(r) = renderer() else { return };
    let path = book("trial");
    let s = scene(&[("trial", entry("return tap(in.uv).bgra;"))]);
    let key = s.loaded["trial"].key;
    let mut shown = show(r, s).unwrap();
    assert_eq!(trials(&path, key), 1, "a built effect is on trial until it has drawn");
    for _ in 0..3 {
        frame(r, &mut shown).unwrap();
    }
    assert_eq!(trials(&path, key), 0);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    std::fs::write(&path, format!("{key:016x} trying 4000000000.1\n{key:016x} 1 {now}\n")).unwrap();
    wprs_scene::keep_strikes(&path);
    let mut shown = show(r, scene(&[("trial", entry("return tap(in.uv).bgra;"))])).unwrap();
    let left = shown.view.left_out();
    assert!(left.len() == 1 && left[0].contains("stopped twice"), "{left:?}");
    frame(r, &mut shown).unwrap();
    assert_eq!(wprs_scene::forgive_strikes().unwrap(), 1);
    assert!(shown.view.retry(), "an effect that was never built needs a new view");
    let mut shown = show(r, scene(&[("trial", entry("return tap(in.uv).bgra;"))])).unwrap();
    assert!(shown.view.left_out().is_empty());
    frame(r, &mut shown).unwrap();
}
