use serde_json::{Value, json};
use std::sync::OnceLock;
use tempfile::TempDir;
use wprs_editor::Project;
use wprs_editor::actions::Action;
use wprs_editor::inspect::fields;
use wprs_editor::starters::starter;

fn act(p: &mut Project, action: Value) -> anyhow::Result<()> {
    p.act(&serde_json::from_value::<Action>(action).unwrap()).map(drop)
}

fn project(root: &TempDir) -> Project {
    static STATE: OnceLock<TempDir> = OnceLock::new();
    STATE.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("XDG_STATE_HOME", dir.path()) };
        dir
    });
    let made = starter("empty", "org.example.cues", "Cues", "en").unwrap();
    let mut p = Project::create(&root.path().join("cues"), made).unwrap();
    let timeline = json!({ "id": "song", "duration": 1, "autoplay": false });
    act(&mut p, json!({ "set": { "pointer": "/scene/timelines", "value": [timeline] } })).unwrap();
    p
}

fn trigger(p: &Project) -> wprs_editor::inspect::Field {
    let found = fields(&p.doc, "/scene/timelines/0").into_iter().find(|f| f.key == "trigger");
    found.expect("a timeline has a trigger field")
}

#[test]
fn the_trigger_picker_offers_media_cues_and_refuses_unknown_ones() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let field = trigger(&p);
    assert_eq!(field.widget, "ref:trigger");
    let media: Vec<&str> =
        field.options.iter().filter_map(Value::as_str).filter(|o| o.starts_with("media:")).collect();
    assert_eq!(media, ["media:play", "media:pause", "media:track", "media:cover"]);
    act(&mut p, json!({ "set": { "pointer": "/scene/timelines/0/trigger", "value": "media:track" } }))
        .unwrap();
    assert_eq!(trigger(&p).value, json!("media:track"));
    let problems = wprs_editor::package::problems(&p.dir, &p.doc);
    assert!(problems.iter().all(|c| c.warning), "{problems:?}");
    act(&mut p, json!({ "set": { "pointer": "/scene/timelines/0/trigger", "value": "media:skip" } }))
        .unwrap();
    let problems = wprs_editor::package::problems(&p.dir, &p.doc);
    assert!(problems.iter().any(|c| c.message.contains("skip")), "{problems:?}");
}

#[test]
fn property_triggers_of_the_manifest_offer_no_media_cues() {
    let doc = json!({ "manifest": { "properties": {} } });
    let options = wprs_editor::settings::candidates(&doc, "trigger", "/manifest/properties/x").unwrap();
    assert!(options.iter().all(|o| !o.as_str().unwrap_or_default().starts_with("media:")), "{options:?}");
}
