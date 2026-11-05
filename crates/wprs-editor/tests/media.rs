use serde_json::{Value, json};
use std::sync::OnceLock;
use tempfile::TempDir;
use wprs_editor::Project;
use wprs_editor::actions::Action;
use wprs_editor::inspect::fields;
use wprs_editor::starters::starter;

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

fn act(p: &mut Project, action: Value) -> anyhow::Result<()> {
    p.act(&serde_json::from_value::<Action>(action).unwrap()).map(drop)
}

#[test]
fn a_text_follows_the_media_player_and_says_so_in_the_inspector() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    act(&mut p, json!({ "set": { "pointer": "/scene/layers/0/text", "value": { "media": "artist" } } }))
        .unwrap();
    let shown = fields(&p.doc, "/scene/layers/0");
    let text = shown.iter().find(|f| f.key == "text").unwrap();
    assert_eq!((text.source.as_str(), text.bindable), ("media", true));
    let problems = wprs_editor::package::problems(&p.dir, &p.doc);
    assert!(problems.iter().all(|c| !c.pointer.contains("/text")), "{problems:?}");
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert_ne!(fields(&p.doc, "/scene/layers/0").iter().find(|f| f.key == "text").unwrap().source, "media");
}

#[test]
fn the_preview_carries_a_made_up_track_with_its_quality() {
    let look = json!({ "quality": "q2", "media": { "title": "Night Drive", "playing": true } });
    let staged = wprs_editor::rigs::staged(&look).unwrap();
    assert_eq!(staged["media"]["title"], "Night Drive");
    assert_eq!(staged["quality"], "q2");
    let only = wprs_editor::rigs::staged(&json!({ "media": { "title": "x" } })).unwrap();
    assert!(only["quality"].is_null() && only["media"].is_object());
    assert!(wprs_editor::rigs::staged(&json!({ "media": "x" })).is_none());
}

#[test]
fn a_picture_becomes_the_cover_with_its_placeholder_and_back() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let file = root.path().join("dark.png");
    image::RgbImage::from_pixel(32, 32, image::Rgb([20, 20, 30])).save(&file).unwrap();
    act(&mut p, json!({ "import": { "file": file, "id": "dark" } })).unwrap();
    let path = p.doc["scene"]["assets"]["dark"]["path"].clone();
    act(&mut p, json!({ "asset": { "asset": "cover", "cover": "current", "placeholder": "dark" } })).unwrap();
    assert_eq!(p.doc["scene"]["assets"]["cover"], json!({ "path": path, "media": "current" }));
    act(&mut p, json!({ "asset": { "asset": "before", "cover": "previous", "placeholder": path } })).unwrap();
    assert_eq!(p.doc["scene"]["assets"]["before"]["media"], "previous");
    let refused = |p: &mut Project, v: Value| act(p, json!({ "asset": v })).unwrap_err().to_string();
    assert!(refused(&mut p, json!({ "asset": "x", "cover": "current" })).contains("placeholder"));
    assert!(
        refused(&mut p, json!({ "asset": "cover", "cover": "next" })).contains("current, previous or none")
    );
    assert!(
        refused(&mut p, json!({ "asset": "cover", "placeholder": "song.ogg" })).contains("still picture")
    );
    assert!(
        refused(&mut p, json!({ "asset": "cover", "placeholder": "../dark.png" })).contains("still picture")
    );
    act(
        &mut p,
        json!({ "set": { "pointer": "/scene/layers/0", "value":
        { "id": "art", "type": "image", "image": "cover", "size": [200, 200] } } }),
    )
    .unwrap();
    let problems = wprs_editor::package::problems(&p.dir, &p.doc);
    assert!(problems.iter().all(|c| c.warning || !c.pointer.contains("assets")), "{problems:?}");
    act(&mut p, json!({ "asset": { "asset": "cover", "cover": "none" } })).unwrap();
    assert!(p.doc["scene"]["assets"]["cover"].get("media").is_none());
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert_eq!(p.doc["scene"]["assets"]["cover"]["media"], "current");
}

#[test]
fn the_palette_of_a_picture_is_shown_as_its_cover_colours() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let file = root.path().join("dusk.png");
    image::RgbImage::from_fn(64, 64, |x, _| image::Rgb(if x < 40 { [20, 20, 60] } else { [250, 230, 180] }))
        .save(&file)
        .unwrap();
    act(&mut p, json!({ "import": { "file": file, "id": "dusk" } })).unwrap();
    let ask = |what: Value| wprs_editor::rigs::query(&p.dir, &p.doc, "", &json!({ "palette": what }));
    let colours = ask(json!("dusk"));
    assert_eq!(colours.as_array().map(Vec::len), Some(5), "{colours}");
    assert_eq!((colours[0].as_str(), colours[3].as_str()), (Some("#14143c"), Some("#fae6b4")));
    let path = p.doc["scene"]["assets"]["dusk"]["path"].clone();
    assert_eq!(ask(path), colours, "a path of the folder works as well as the asset id");
    assert!(ask(json!("missing.png")).is_null() && ask(json!(null)).is_null());
}

#[test]
fn choosing_the_cover_for_a_field_never_turns_a_plain_picture_into_one_and_undoes_in_one_step() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let file = root.path().join("dark.png");
    image::RgbImage::from_pixel(32, 32, image::Rgb([20, 20, 30])).save(&file).unwrap();
    act(&mut p, json!({ "import": { "file": file.clone(), "id": "cover" } })).unwrap();
    act(
        &mut p,
        json!({ "set": { "pointer": "/scene/layers/0", "value":
        { "id": "art", "type": "image", "image": "cover", "size": [64, 64] } } }),
    )
    .unwrap();
    let plain = p.doc["scene"]["assets"]["cover"].clone();
    let field = "/scene/layers/0/image";
    let pick = |kind: &str| json!({ "asset": { "asset": "cover", "cover": kind, "placeholder": "cover", "field": field } });
    act(&mut p, pick("current")).unwrap();
    assert_eq!(p.doc["scene"]["assets"]["cover"], plain, "the plain picture called cover stays plain");
    let made = p.doc.pointer(field).and_then(Value::as_str).unwrap().to_owned();
    assert_ne!(made, "cover");
    assert_eq!(p.doc["scene"]["assets"][&made], json!({ "path": plain["path"], "media": "current" }));
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert_eq!(p.doc.pointer(field), Some(&json!("cover")), "one undo takes the field back");
    assert!(p.doc["scene"]["assets"].get(&made).is_none(), "and the cover asset it made");
    act(&mut p, json!({ "redo": {} })).unwrap();
    act(&mut p, json!({ "set": { "pointer": field, "value": "cover" } })).unwrap();
    act(&mut p, pick("current")).unwrap();
    assert_eq!(p.doc.pointer(field), Some(&json!(made)), "an existing cover of that kind is used again");
    let count = p.doc["scene"]["assets"].as_object().unwrap().len();
    act(&mut p, pick("previous")).unwrap();
    let previous = p.doc.pointer(field).and_then(Value::as_str).unwrap().to_owned();
    assert_eq!(p.doc["scene"]["assets"][&previous]["media"], "previous");
    assert_eq!(p.doc["scene"]["assets"].as_object().unwrap().len(), count + 1);
    let refused = |p: &mut Project, v: Value| act(p, json!({ "asset": v })).unwrap_err().to_string();
    let bad = |at: &str| json!({ "asset": "cover", "cover": "current", "field": at });
    assert!(refused(&mut p, bad("/manifest/title")).contains("not a field of the scene"));
    assert!(refused(&mut p, bad("/scene/assets/cover/path")).contains("not a field of the scene"));
    assert!(refused(&mut p, bad("/scene/layers/9/image")).contains("not a field of the scene"));
    let none = json!({ "asset": "cover", "cover": "none", "field": field });
    assert!(refused(&mut p, none).contains("current or the previous cover"));
}
