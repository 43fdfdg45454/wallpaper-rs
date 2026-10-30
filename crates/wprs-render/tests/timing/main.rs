mod again;
mod blame;
mod exits;
#[path = "../hostile/mod.rs"]
mod hostile;
mod slow;
mod spans;
mod stack;
mod variants;

use hostile::{Shown, entry};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};
use wprs_render::Renderer;
use wprs_scene::Quality;

static ONE: Mutex<()> = Mutex::new(());

pub fn one() -> MutexGuard<'static, ()> {
    let one = ONE.lock().unwrap_or_else(PoisonError::into_inner);
    wprs_render::set_gpu_budget(None);
    wprs_render::set_build_timeout(Duration::from_secs(10));
    wprs_render::retry_effects();
    one
}

pub fn book(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    drop(std::fs::remove_dir_all(&dir));
    let path = dir.join("stopped-effects");
    wprs_scene::keep_strikes(&path);
    path
}

pub fn lines(path: &PathBuf, key: u64, kind: &str) -> usize {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    text.lines().filter(|l| l.starts_with(&format!("{key:016x} {kind} "))).count()
}

pub fn busy(steps: usize) -> String {
    let step = "v = fract(v * 1.37 + vec4<f32>(0.13));".repeat(steps);
    entry(&format!(
        "var v = tap(in.uv) + vec4<f32>(frame.time * 1e-3); for (var i = 0u; i < 1024u; i++) {{ {step} }} return v;"
    ))
}

pub fn wide(r: &Renderer) -> (u32, u32) {
    if r.software() { (512, 256) } else { (1920, 1080) }
}

pub fn draw(r: &Renderer, shown: &mut Shown, size: (u32, u32)) -> Duration {
    shown.runtime.advance(1.0 / 60.0);
    let frame = shown.runtime.frame(shown.view.view(size), Quality::Q3);
    let started = Instant::now();
    r.render_scene(&mut shown.view, &shown.runtime, &frame, size, Quality::Q3, &[]).unwrap();
    started.elapsed()
}

pub fn slow_build() -> String {
    let mut s = String::from("fn f0(m: mat4x4<f32>) -> mat4x4<f32> { var a = m; let n = m * 0.999; ");
    s.push_str(&"a = a * n;".repeat(200));
    s.push_str(" return a; }\nfn f1(m: mat4x4<f32>) -> mat4x4<f32> { var a = m; ");
    s.push_str(&"a = f0(a);".repeat(8));
    let salt = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos();
    let body = format!("let v = tap(in.uv) * f32({salt}u); return f1(mat4x4<f32>(v, v, v, v))[0];");
    s + " return a; }\n" + &entry(&body)
}
