use serde_json::{Value, json};
use std::fs;
use wprs_editor::Project;
use wprs_editor::actions::Action;
use wprs_editor::inspect::{fields_at, fields_with};
use wprs_editor::outline::outline;
use wprs_editor::starters::starter;

const WGSL: &str = "override soft: bool = false;
@fragment fn fs_glow(in: VertexOut) -> @location(0) vec4<f32> {
    return tap(in.uv) * select(param(0u).x, 1.0, soft);
}";

fn descriptor(id: &str) -> Value {
    json!({ "id": id, "name": { "en": "Glow", "es": "Brillo" }, "cost": { "taps": 1 }, "passes": [{ "entry": "fs_glow" }],
        "params": [{ "id": "gain", "type": "float", "default": 1, "min": 0, "max": 4,
            "label": { "en": "Gain", "es": "Ganancia" } },
            { "id": "style", "type": "enum", "default": 0, "values": [{ "value": 0, "label": { "en": "Soft" } }, { "value": 1, "label": { "en": "Hard" } }] },
            { "id": "shape", "type": "texture", "default": "", "help": { "en": "What the glow follows" } }],
        "options": [{ "id": "soft", "values": [false, true], "default": false }] })
}

#[test]
fn package_effects_are_imported_listed_and_inspected() {
    let root = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("XDG_STATE_HOME", root.path().join("state")) };
    let dir = root.path().join("glow");
    let mut project =
        Project::create(&dir, starter("empty", "org.example.glow", "Glow", "en").unwrap()).unwrap();
    let source = root.path().join("source");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("glow.json"), descriptor("glow").to_string()).unwrap();
    fs::write(source.join("glow.wgsl"), WGSL).unwrap();
    fs::write(source.join("blur.json"), descriptor("blur").to_string()).unwrap();
    fs::write(source.join("loop.json"), descriptor("loop").to_string()).unwrap();
    fs::write(source.join("loop.wgsl"), WGSL.replace("return", "loop { }\n    return")).unwrap();
    let import = |p: &mut Project, name: &str, wgsl: Option<&str>| {
        let wgsl = wgsl.map(|w| source.join(w));
        p.act(&Action::ImportEffect { descriptor: source.join(name), wgsl })
    };
    import(&mut project, "glow.json", None).unwrap();
    assert_eq!(
        project.doc["scene"]["effects"]["glow"],
        json!({ "descriptor": "effects/glow.json", "wgsl": "effects/glow.wgsl" })
    );
    assert!(dir.join("effects/glow.wgsl").is_file());
    let blur = import(&mut project, "blur.json", Some("glow.wgsl")).unwrap_err().to_string();
    assert!(blur.contains("blur is the name of a built-in effect"), "{blur}");
    let looped = import(&mut project, "loop.json", None).unwrap_err().to_string();
    assert!(looped.contains("a counter that a literal limits"), "{looped}");
    let listed = outline(&project, 0.0, &[]);
    assert!(listed["effects"].as_array().unwrap().contains(&json!("glow")));
    let layer = json!({"add_layer": {"kind": "solid", "id": "fill"}});
    project.act(&serde_json::from_value::<Action>(layer).unwrap()).unwrap();
    let pointer = wprs_editor::layers::find(&project.doc, "fill").unwrap();
    let effect = json!({"add": {"pointer": format!("{pointer}/effects"), "value": [{ "effect": "glow", "params": { "gain": 2 } }]}});
    project.act(&serde_json::from_value::<Action>(effect).unwrap()).unwrap();
    let at = format!("{pointer}/effects/0");
    assert!(fields_at(&project.doc, &at, 0.0).iter().all(|f| f.key != "gain"));
    let fields = fields_with(&project.doc, &at, 0.0, &project.effects());
    let gain = fields.iter().find(|f| f.key == "gain").unwrap();
    assert_eq!((gain.title.as_str(), &gain.range, &gain.shown), ("Gain", &json!([0.0, 4.0]), &json!(2)));
    let style = fields.iter().find(|f| f.key == "style").unwrap();
    assert_eq!(
        (style.widget.as_str(), style.names.clone()),
        ("enum", vec!["Soft".to_string(), "Hard".into()])
    );
    let shape = fields.iter().find(|f| f.key == "shape").unwrap();
    assert_eq!((shape.widget.as_str(), shape.help.as_str()), ("ref:image", "What the glow follows"));
    assert!(!shape.options.contains(&json!({ "layer": "fill" })), "{:?}", shape.options);
    assert!(["white", "black", "noise", "normal_flat"].iter().all(|g| shape.options.contains(&json!(g))));
    let soft = fields.iter().find(|f| f.key == "soft").unwrap();
    assert_eq!(
        (soft.widget.as_str(), &soft.options, soft.bindable),
        ("enum", &vec![json!(false), json!(true)], true)
    );
    assert_eq!(outline(&project, 0.0, &[])["effect_names"]["glow"], "Glow");
    let other = json!({"add_layer": {"kind": "solid", "id": "light"}});
    project.act(&serde_json::from_value::<Action>(other).unwrap()).unwrap();
    let pointer = wprs_editor::layers::find(&project.doc, "fill").unwrap();
    let at = format!("{pointer}/effects/0");
    let fields = fields_with(&project.doc, &at, 0.0, &project.effects());
    let shape = fields.iter().find(|f| f.key == "shape").unwrap();
    assert!(shape.options.contains(&json!({ "layer": "light" })), "{:?}", shape.options);
    let calm = json!({"add_property": {"kind": "bool", "id": "calm"}});
    project.act(&serde_json::from_value::<Action>(calm).unwrap()).unwrap();
    for (key, value) in [("shape", json!({ "layer": "light" })), ("soft", json!({ "prop": "calm" }))] {
        let set = json!({"set": {"pointer": format!("{at}/params/{key}"), "value": value}});
        project.act(&serde_json::from_value::<Action>(set).unwrap()).unwrap();
    }
    let fields = fields_with(&project.doc, &at, 0.0, &project.effects());
    let found = |key: &str| fields.iter().find(|f| f.key == key).unwrap().clone();
    assert_eq!((found("shape").value, found("soft").source), (json!({ "layer": "light" }), "prop".into()));
    assert!(
        wprs_editor::check::review(&project.doc, Some(&dir)).iter().all(|p| p.warning),
        "{:?}",
        wprs_editor::check::review(&project.doc, Some(&dir))
    );
}
