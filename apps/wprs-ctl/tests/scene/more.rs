use super::*;

#[test]
fn scene_upgrade_takes_what_pack_upgrade_takes() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    for name in ["old", "older"] {
        ok(dir, &["scene", "new", name, "--from", "particles"]);
        ok(dir, &["scene", "set", name, "/manifest/schema", "wallpaper-rs.wpak/1"]);
        for field in ["content", "photosensitive", "origin", "default_locale"] {
            ok(dir, &["scene", "remove", name, &format!("/manifest/{field}")]);
        }
    }
    ok(dir, &["scene", "validate", "old"]);
    ok(dir, &["scene", "upgrade", "old", "--rating", "general", "--photosensitive", "false"]);
    ok(dir, &["scene", "validate", "old"]);
    assert_eq!(
        (show_in(dir, "old", "/manifest/default_locale"), show_in(dir, "old", "/manifest/origin")),
        (json!("en"), json!({"kind": "original"}))
    );
    ok(dir, &["scene", "upgrade", "older"]);
    let listed = String::from_utf8(run(dir, &["scene", "validate", "older"]).stdout).unwrap();
    for field in ["content", "photosensitive", "origin"] {
        assert!(listed.contains(&format!("/manifest/{field}: {field}: is required")), "{listed}");
    }
    ok(dir, &["scene", "undo", "older"]);
    assert_eq!(show_in(dir, "older", "/manifest/schema"), json!("wallpaper-rs.wpak/1"));
}

#[test]
fn user_properties_and_presets_are_edited_from_the_command_line() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    ok(dir, &["scene", "new", "snow", "--from", "particles"]);
    assert_eq!(
        ok(dir, &["scene", "add-property", "snow", "group", "--id", "look", "--label", "Look"]),
        "look\n"
    );
    assert_eq!(ok(dir, &["scene", "add-property", "snow", "combo", "--group", "look"]), "combo_1\n");
    ok(dir, &["scene", "rename-property", "snow", "combo_1", "mood"]);
    assert_eq!(ok(dir, &["scene", "add-property", "snow", "button"]), "button_1\n");
    assert!(fails(dir, &["scene", "condition", "snow", "button_1", "mood =="]).contains("ends too early"));
    ok(dir, &["scene", "condition", "snow", "button_1", "mood == 'option_2'", "--enabled"]);
    ok(dir, &["scene", "move-property", "snow", "button_1", "0", "--group", "look"]);
    assert_eq!(
        show(dir, "/manifest/properties/button_1"),
        json!({"type": "button", "group": "look", "order": 1, "enabled_if": "mood == 'option_2'"})
    );
    assert_eq!(show(dir, "/manifest/properties/mood/order"), 2);
    assert_eq!(ok(dir, &["scene", "add-preset", "snow", "--label", "Two"]), "preset_1\n");
    ok(dir, &["scene", "set", "snow", "/manifest/presets/0/values/mood", "option_2"]);
    assert!(show(dir, "/manifest/requires/features").as_array().unwrap().contains(&json!("properties2")));
    let fields: Value =
        serde_json::from_str(&ok(dir, &["scene", "fields", "snow", "/manifest/presets/0"])).unwrap();
    assert!(fields.as_array().unwrap().iter().any(|f| f["key"] == "mood" && f["set"] == true));
    assert_eq!(ok_lines(dir), "");
    std::fs::create_dir_all(dir.join("snow/scripts")).unwrap();
    std::fs::write(dir.join("snow/scripts/look.js"), "export const update = () => props.get('mood');")
        .unwrap();
    ok(dir, &["scene", "set", "snow", "/scene/scripts", r#"[{"path": "scripts/look.js"}]"#]);
    let renamed = run(dir, &["scene", "rename-property", "snow", "mood", "weather"]);
    let warned = String::from_utf8(renamed.stderr).unwrap();
    assert!(renamed.status.success() && warned.contains("scripts/look.js names \"mood\""), "{warned}");
    let named = fails(dir, &["scene", "remove-property", "snow", "weather"]);
    assert!(named.contains("/manifest/properties/button_1/enabled_if"), "{named}");
    ok(dir, &["scene", "condition", "snow", "button_1", "", "--enabled"]);
    ok(dir, &["scene", "remove-property", "snow", "weather"]);
    assert_eq!(show(dir, "/manifest/presets/0/values"), json!({}));
    let scene = Cli::command().find_subcommand("scene").cloned().unwrap();
    for name in
        ["add-property", "move-property", "rename-property", "remove-property", "condition", "add-preset"]
    {
        let command = scene.find_subcommand(name).unwrap();
        let bare: Vec<_> =
            command.get_arguments().filter(|a| a.get_help().is_none()).map(|a| a.get_id()).collect();
        assert!(bare.is_empty(), "{name} does not explain {bare:?}");
    }
}

fn words(line: &str) -> Vec<String> {
    let (mut out, mut word, mut quote, mut started) = (Vec::new(), String::new(), None, false);
    for c in line.trim().chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => word.push(c),
            (None, '\'' | '"') => (quote, started) = (Some(c), true),
            (None, ' ') => {
                if started {
                    out.push(std::mem::take(&mut word));
                }
                started = false;
            }
            (None, c) => {
                word.push(c);
                started = true;
            }
        }
    }
    out.extend(started.then_some(word));
    out
}

#[test]
fn the_scene_commands_of_the_readme_work_as_written() {
    let readme = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../README.md")).unwrap();
    let elsewhere =
        ["open", "import", "add-layer", "thumbnail", "upgrade", "preview", "build", "apply", "bench"];
    let tmp = tempfile::tempdir().unwrap();
    let lines = readme.lines().filter(|l| l.starts_with("wprs-ctl scene "));
    let commands = lines.flat_map(|l| l.split("  #").next().unwrap_or_default().split(" && ")).map(words);
    let mut ran = 0;
    for args in commands.filter(|a| a[3] == "mar" && !elsewhere.contains(&a[2].as_str())) {
        ok(tmp.path(), &args[1..].iter().map(String::as_str).collect::<Vec<_>>());
        ran += 1;
    }
    assert!(ran >= 18, "only {ran} commands of the README ran");
    assert_eq!(
        show_in(tmp.path(), "mar", "/manifest/properties/velocidad/label"),
        json!({"en": "Speed", "es": "Velocidad"})
    );
}

#[test]
fn package_effects_are_imported_from_the_command_line() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    ok(dir, &["scene", "new", "snow", "--from", "empty", "--title", "Snow"]);
    let descriptor = json!({ "id": "dim", "cost": {}, "passes": [{ "entry": "fs_dim" }] });
    std::fs::write(dir.join("dim.json"), descriptor.to_string()).unwrap();
    std::fs::write(
        dir.join("dim.wgsl"),
        "@fragment fn fs_dim(in: VertexOut) -> @location(0) vec4<f32> { return tap(in.uv) * 0.5; }",
    )
    .unwrap();
    ok(dir, &["scene", "import-effect", "snow", "dim.json"]);
    assert_eq!(show(dir, "/scene/effects/dim/wgsl"), "effects/dim.wgsl");
    std::fs::write(
        dir.join("dim.wgsl"),
        "@fragment fn fs_other(in: VertexOut) -> @location(0) vec4<f32> { return tap(in.uv); }",
    )
    .unwrap();
    assert!(
        fails(dir, &["scene", "import-effect", "snow", "dim.json"]).contains("has no entry point fs_dim")
    );
}

#[test]
fn scripts_are_imported_with_what_they_declare() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    ok(dir, &["scene", "new", "snow", "--from", "empty", "--title", "Snow"]);
    let module = r#"import { scene, audio, setInterval, sound } from "wprs";
        import { Vec2 } from "wprs/math";
        export function init(ctx) { scene.spawn("bar"); scene.layer("bar_1").on("click", () => {}); }
        export const spin = (value, ctx) => value + 1;
        export function update() {}"#;
    std::fs::write(dir.join("bars.js"), module).unwrap();
    ok(dir, &["scene", "import-script", "snow", "bars.js"]);
    let entry = json!({ "path": "scripts/bars.js", "events": ["init", "update", "click"], "uses": ["audio", "timers", "layers", "sounds", "math"] });
    assert_eq!(show(dir, "/scene/scripts"), json!([entry]));
    assert!(dir.join("snow/scripts/bars.js").is_file());
    std::fs::write(dir.join("big.js"), " ".repeat((1 << 20) + 1)).unwrap();
    assert!(fails(dir, &["scene", "import-script", "snow", "big.js"]).contains("1 MiB"));
    assert!(fails(dir, &["scene", "import-script", "snow", "missing.js"]).contains("cannot read"));
}
