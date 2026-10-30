use serde_json::{Value, json};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

const GOOD: &str =
    "@fragment fn fs_dim(in: VertexOut) -> @location(0) vec4<f32> { return tap(in.uv) * 0.5; }";
const ENDLESS: &str = "@fragment fn fs_dim(in: VertexOut) -> @location(0) vec4<f32> {
    var d = tap(in.uv);
    loop { d *= 0.5; if d.a < -1.0 { break; } }
    return d;
}";
const REASON: &str = "a counter that a literal limits";

fn descriptor() -> String {
    json!({ "id": "dim", "cost": {}, "passes": [{ "entry": "fs_dim" }] }).to_string()
}

fn run(dir: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_wprs-ctl"));
    command.args(args).current_dir(dir).stdin(Stdio::null()).env("LC_ALL", "C");
    command.env("XDG_STATE_HOME", dir.join("state")).env("XDG_CACHE_HOME", dir.join("cache"));
    command.env("DBUS_SESSION_BUS_ADDRESS", format!("unix:path={}/no-bus", dir.display()));
    command.output().unwrap()
}

fn said(out: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

fn asked(wgsl: &str) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_wprs-ctl"))
        .arg("--wprs-check-shader")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let request = json!({ "id": "dim", "json": descriptor(), "wgsl": wgsl }).to_string();
    child.stdin.take().unwrap().write_all(request.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{}", said(&out));
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn the_shader_checker_answers_on_its_marker() {
    let good = asked(GOOD);
    assert!(good["hash"].as_str().is_some_and(|h| h.len() == 64) && good["refused"].is_null(), "{good}");
    let hostile = asked(ENDLESS);
    assert!(hostile["refused"].as_str().is_some_and(|r| r.contains(REASON)), "{hostile}");
}

#[test]
fn hostile_package_shaders_are_refused_by_the_checker() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let made = run(dir, &["scene", "new", "snow", "--from", "empty", "--title", "Snow"]);
    assert!(made.status.success(), "{}", said(&made));
    std::fs::write(dir.join("dim.json"), descriptor()).unwrap();
    std::fs::write(dir.join("dim.wgsl"), ENDLESS).unwrap();
    let refused = run(dir, &["scene", "import-effect", "snow", "dim.json"]);
    assert!(!refused.status.success() && said(&refused).contains(REASON), "{}", said(&refused));
    let kept = dir.join("cache/wallpaper-rs/checked-effects");
    let lines = || std::fs::read_to_string(&kept).unwrap_or_default().lines().map(String::from).collect();
    let first: Vec<String> = lines();
    assert!(first.len() == 1 && first[0].contains(" refused ") && first[0].contains(REASON), "{first:?}");
    std::fs::write(dir.join("dim.wgsl"), GOOD).unwrap();
    let imported = run(dir, &["scene", "import-effect", "snow", "dim.json"]);
    assert!(imported.status.success(), "{}", said(&imported));
    std::fs::write(dir.join("snow/effects/dim.wgsl"), ENDLESS).unwrap();
    let checked = run(dir, &["pack", "check", "snow"]);
    assert!(!checked.status.success() && said(&checked).contains(REASON), "{}", said(&checked));
    let after: Vec<String> = lines();
    assert!(after.len() == 2 && after[1].ends_with(" ok") && after[0] == first[0], "{after:?}");
}
