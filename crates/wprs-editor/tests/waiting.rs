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
    let made = starter("empty", "org.example.tune", "Tune", "en").unwrap();
    let mut p = Project::create(&root.path().join("tune"), made).unwrap();
    act(&mut p, json!({ "add_layer": { "kind": "text", "id": "title" } })).unwrap();
    p
}

#[test]
fn the_text_while_nothing_plays_is_a_text_in_every_language() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let value = json!({ "media": "title", "placeholder": "Nothing plays" });
    act(&mut p, json!({ "set": { "pointer": "/scene/layers/0/text", "value": value } })).unwrap();
    let shown = fields(&p.doc, "/scene/layers/0/text");
    let field = shown.iter().find(|f| f.key == "placeholder").unwrap();
    assert_eq!(field.widget, "texts");
    let at = "/scene/layers/0/text/placeholder";
    let translate = |language: &str, text: Value| json!({ "translate": { "pointer": at, "language": language, "text": text } });
    act(&mut p, translate("es", json!("No suena nada"))).unwrap();
    assert_eq!(p.doc.pointer(at).unwrap(), &json!({ "en": "Nothing plays", "es": "No suena nada" }));
    let problems = wprs_editor::package::problems(&p.dir, &p.doc);
    assert!(problems.iter().all(|c| !c.pointer.contains("/text")), "{problems:?}");
    act(&mut p, translate("es", Value::Null)).unwrap();
    assert_eq!(p.doc.pointer(at).unwrap(), &json!({ "en": "Nothing plays" }));
    act(&mut p, json!({ "set": { "pointer": "/scene/layers/0/text", "value": { "media": "progress", "placeholder": "x" } } }))
        .unwrap();
    let problems = wprs_editor::package::problems(&p.dir, &p.doc);
    assert!(problems.iter().any(|c| c.pointer.ends_with("/placeholder")), "{problems:?}");
    let other = json!({ "translate": { "pointer": "/scene/layers/0/id", "language": "es", "text": "x" } });
    assert!(act(&mut p, other).is_err(), "only texts are translated");
}
