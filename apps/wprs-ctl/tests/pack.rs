use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_wprs-ctl")).args(args).output().unwrap()
}

fn ok(args: &[&str]) -> String {
    let out = run(args);
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn check_explains_what_a_package_needs_and_upgrade_rates_it() {
    let pulse = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenes/pulse");
    let shown = ok(&["pack", "check", pulse.to_str().unwrap()]);
    for line in [
        "rating = general",
        "photosensitive = yes",
        "inputs = audio",
        "needs = inputs.audio (reacts to the sound playing on the computer)",
        "languages = en, es",
    ] {
        assert!(shown.lines().any(|l| l == line), "{line}: {shown}");
    }
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    std::fs::create_dir(&src).unwrap();
    std::fs::copy(pulse.join("scene.json"), src.join("scene.json")).unwrap();
    let manifest = r##"{"schema": "wallpaper-rs.wpak/1", "id": "org.example.beat", "version": "1.0.0",
        "title": "Beat", "license": "CC0-1.0", "kind": "scene", "entry": "scene.json",
        "properties": {"color": {"type": "color", "default": "#ffffff"}, "show": {"type": "combo",
        "default": "both", "options": ["circle", "bars", "both"]}, "glitch": {"type": "bool", "default": true}}}"##;
    std::fs::write(src.join("manifest.json"), manifest).unwrap();
    let path = src.to_str().unwrap();
    let old: Value = serde_json::from_str(&ok(&["pack", "check", path, "--json"])).unwrap();
    assert_eq!((&old["rating"], &old["missing"]), (&Value::Null, &serde_json::json!([])));
    let refused = run(&["pack", "upgrade", path, "--rating", "mature", "--photosensitive", "true"]);
    assert!(String::from_utf8_lossy(&refused.stderr).contains("content.descriptors"));
    ok(&["pack", "upgrade", path, "--rating", "mature", "--reason", "violence", "--photosensitive", "true"]);
    let new: Value = serde_json::from_str(&ok(&["pack", "check", path, "--json"])).unwrap();
    assert_eq!(
        (&new["schema"], &new["rating"], &new["reasons"], &new["inputs"], &new["declared"]),
        (
            &serde_json::json!("wallpaper-rs.wpak/2"),
            &serde_json::json!("mature"),
            &serde_json::json!(["violence"]),
            &serde_json::json!(["audio"]),
            &serde_json::json!(["inputs.audio"])
        )
    );
    std::fs::create_dir(src.join("locale")).unwrap();
    std::fs::write(src.join("locale/fr.json"), r#"{"Good morning": "Bonjour"}"#).unwrap();
    let built = tmp.path().join("beat.wpak");
    let out = run(&["pack", "build", path, "-o", built.to_str().unwrap()]);
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success() && said.starts_with("note: locale/fr.json ") && !said.contains("+="),
        "{said}"
    );
    assert!(ok(&["pack", "check", built.to_str().unwrap()]).contains("rating = mature (violence)"));
}

#[test]
fn paths_of_file_and_folder_settings_become_absolute() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().canonicalize().unwrap();
    std::fs::create_dir_all(base.join("2024/pics")).unwrap();
    let path = |text: &str| wprs_ctl::props::path(text, &base).unwrap();
    let quoted = |p: &Path| serde_json::to_string(p.to_str().unwrap()).unwrap();
    assert_eq!(path("2024"), quoted(&base.join("2024")));
    assert_eq!(path(r#""2024/pics""#), quoted(&base.join("2024/pics")));
    assert_eq!(path("/srv/a b.png"), r#""/srv/a b.png""#);
    assert!(wprs_ctl::props::path("missing.png", &base).is_err());
    for empty in ["", " ", r#""""#, r#"" ""#] {
        assert!(wprs_ctl::props::path(empty, &base).is_err(), "{empty:?}");
    }
}
