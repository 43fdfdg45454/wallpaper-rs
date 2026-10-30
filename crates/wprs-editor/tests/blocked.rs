use serde_json::json;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;
use wprs_editor::Project;
use wprs_editor::check::{Problem, review};
use wprs_editor::starters::starter;

const JSON: &str = r#"{ "id": "dim", "cost": {}, "passes": [{ "entry": "fs_dim" }] }"#;
static ONE: Mutex<()> = Mutex::new(());
const WGSL: &str =
    "@fragment fn fs_dim(in: VertexOut) -> @location(0) vec4<f32> { return tap(in.uv) * 0.5; }";

fn dim(dir: &Path, wgsl: &str) -> Project {
    let mut project =
        Project::create(dir, starter("empty", "org.example.dim", "Dim", "en").unwrap()).unwrap();
    fs::create_dir_all(dir.join("effects")).unwrap();
    fs::write(dir.join("effects/dim.json"), JSON).unwrap();
    fs::write(dir.join("effects/dim.wgsl"), wgsl).unwrap();
    project.doc["scene"]["effects"] =
        json!({ "dim": { "descriptor": "effects/dim.json", "wgsl": "effects/dim.wgsl" } });
    project
}

#[test]
fn an_effect_blocked_while_open_is_checked_again_once_forgiven() {
    let _one = ONE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("dim");
    let project = dim(&dir, WGSL);
    let checked = wprs_scene::Descriptor::packaged("dim", JSON, WGSL).unwrap().key & !1;
    let book = root.path().join("stopped-effects");
    fs::write(&book, format!("{checked:016x} 2 1\n")).unwrap();
    wprs_scene::keep_strikes(&book);
    let stopped = |p: &Problem| p.message.contains(wprs_scene::STOPPED);
    let first = review(&project.doc, Some(&dir));
    assert!(first.iter().any(stopped), "{first:?}");
    assert_eq!(wprs_scene::forgive_strikes().unwrap(), 1);
    let problems = review(&project.doc, Some(&dir));
    assert!(!problems.iter().any(stopped), "{problems:?}");
}

#[test]
fn a_shader_the_checker_could_not_check_is_checked_again_next_time() {
    let _one = ONE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("dim");
    let project = dim(&dir, &WGSL.replace("0.5", "0.25"));
    wprs_scene::keep_strikes(&root.path().join("stopped-effects"));
    let wall = Duration::from_secs(10);
    let broken = wprs_scene::Checker { program: "/bin/false".into(), memory: 1 << 30, cpu: 4, wall };
    wprs_scene::set_checker(Some(broken));
    let unchecked = |p: &Problem| p.message.contains(wprs_scene::UNCHECKED);
    let first = review(&project.doc, Some(&dir));
    wprs_scene::set_checker(None);
    assert!(first.iter().any(unchecked), "{first:?}");
    let again = review(&project.doc, Some(&dir));
    assert!(!again.iter().any(unchecked), "{again:?}");
}
