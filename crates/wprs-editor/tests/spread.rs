use serde_json::{Value, json};
use std::sync::OnceLock;
use tempfile::TempDir;
use wprs_editor::Project;
use wprs_editor::actions::Action;
use wprs_editor::starters::starter;

fn project(root: &TempDir) -> Project {
    static STATE: OnceLock<TempDir> = OnceLock::new();
    STATE.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("XDG_STATE_HOME", dir.path()) };
        dir
    });
    Project::create(&root.path().join("wide"), starter("empty", "org.example.wide", "Wide", "en").unwrap())
        .unwrap()
}

fn act(p: &mut Project, action: Value) -> anyhow::Result<()> {
    p.act(&serde_json::from_value::<Action>(action).unwrap()).map(drop)
}

fn ask(p: &Project, q: Value) -> Value {
    wprs_editor::rigs::query(&p.dir, &p.doc, "", &json!({ "spread": q }))
}

#[test]
fn the_span_preview_frames_each_simulated_screen_on_the_canvas() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    act(&mut p, json!({ "set": { "pointer": "/scene/canvas/size", "value": [3840, 1080] } })).unwrap();
    let dual = ask(&p, json!({ "layout": "dual", "bezel": 10 }));
    assert_eq!(dual["canvas"], json!([3840, 1080]));
    let parts = dual["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 2);
    let (a, b) = (&parts[0]["rect"], &parts[1]["rect"]);
    assert!(a[0].as_f64().unwrap() < b[0].as_f64().unwrap());
    assert!((a[1].as_f64().unwrap() - b[1].as_f64().unwrap()).abs() < 1e-3);
    let gap = b[0].as_f64().unwrap() - a[0].as_f64().unwrap() - a[2].as_f64().unwrap();
    assert!(gap > 0.0, "the frames hide a strip of the canvas: {dual}");
    assert_eq!(ask(&p, json!({ "layout": "triple" }))["parts"].as_array().unwrap().len(), 3);
    let tall = ask(&p, json!({ "layout": "vertical" }));
    assert!(tall["parts"][1]["rect"][1].as_f64().unwrap() > tall["parts"][0]["rect"][1].as_f64().unwrap());
    assert_eq!(dual["spread"], json!(false));
}

#[test]
fn the_real_layout_is_checked_before_it_is_drawn() {
    let root = tempfile::tempdir().unwrap();
    let p = project(&root);
    let mine = json!([{ "id": "left", "mm": [0, 0, 600, 340] }, { "id": "right", "mm": [620, 0, 600, 340] }]);
    let real = ask(&p, json!({ "layout": "mine", "monitors": mine }));
    assert_eq!(real["parts"][1]["id"], json!("right"));
    assert_eq!(real["bounds"], json!([0.0, 0.0, 1220.0, 340.0]));
    let many: Vec<Value> = (0..65).map(|i| json!({ "id": format!("s{i}"), "mm": [0, 0, 10, 10] })).collect();
    for bad in [
        json!({ "layout": "quad" }),
        json!({ "layout": "mine" }),
        json!({ "layout": "mine", "monitors": [] }),
        json!({ "layout": "mine", "monitors": [{ "id": "a", "mm": [0, 0, 0, 10] }] }),
        json!({ "layout": "mine", "monitors": [{ "id": "a", "mm": [0, 0, 10] }] }),
        json!({ "layout": "mine", "monitors": [{ "mm": [0, 0, 10, 10] }] }),
        json!({ "layout": "mine", "monitors": many }),
    ] {
        let reply = ask(&p, bad.clone());
        assert!(reply["error"].is_string() && reply["parts"] == json!([]), "{bad}: {reply}");
    }
}

#[test]
fn spreading_the_scene_is_an_undoable_manifest_change() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let before = p.doc.clone();
    let pointer = "/manifest/display/span";
    act(&mut p, json!({ "set": { "pointer": pointer, "value": "all-outputs" } })).unwrap();
    assert_eq!(ask(&p, json!({ "layout": "dual" }))["spread"], json!(true));
    let spread = p.doc.clone();
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert_eq!(p.doc, before);
    act(&mut p, json!({ "redo": {} })).unwrap();
    assert_eq!(p.doc, spread);
    act(&mut p, json!({ "remove": { "pointer": pointer } })).unwrap();
    assert_eq!(ask(&p, json!({ "layout": "dual" }))["spread"], json!(false));
    let problems = wprs_editor::package::problems(&p.dir, &p.doc);
    assert!(problems.iter().all(|f| f.warning), "{problems:?}");
    act(&mut p, json!({ "set": { "pointer": pointer, "value": "all-outputs" } })).unwrap();
    let problems = wprs_editor::package::problems(&p.dir, &p.doc);
    assert!(problems.iter().all(|f| f.warning), "a spread scene is a valid package: {problems:?}");
}

#[test]
fn simulated_screens_take_the_size_and_frames_the_editor_picks() {
    let root = tempfile::tempdir().unwrap();
    let p = project(&root);
    let screens = |q: Value| ask(&p, q)["screens"].clone();
    let wide = screens(json!({ "layout": "dual", "bezel": 0, "screen": "34in@21:9" }));
    assert!((wide[0]["mm"][2].as_f64().unwrap() - 793.8).abs() < 0.5, "{wide}");
    let framed = screens(json!({ "layout": "dual", "bezel": 20, "screen": "600x340" }));
    assert_eq!(framed[1]["mm"], json!([640.0, 0.0, 600.0, 340.0]));
    assert_eq!(screens(json!({ "layout": "dual" }))[0]["mm"], json!([0.0, 0.0, 527.0, 296.0]));
    for bad in ["10x10", "24", "34in@21", "nan x 3"] {
        let reply = ask(&p, json!({ "layout": "dual", "screen": bad }));
        assert!(reply["error"].is_string() && reply["parts"] == json!([]), "{bad}: {reply}");
    }
    let mine = json!([{ "id": "left", "mm": [0, 0, 600, 340] }]);
    let real = ask(&p, json!({ "layout": "mine", "monitors": mine, "screen": "10x10" }));
    assert_eq!(
        real["parts"][0]["id"],
        json!("left"),
        "the size of simulated screens never touches the real ones"
    );
}
