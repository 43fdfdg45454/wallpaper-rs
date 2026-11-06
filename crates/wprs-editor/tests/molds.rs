use serde_json::{Value, json};
use std::sync::OnceLock;
use tempfile::TempDir;
use wprs_editor::Project;
use wprs_editor::actions::Action;
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
    let made = starter("empty", "org.example.molds", "Molds", "en").unwrap();
    Project::create(&root.path().join("molds"), made).unwrap()
}

#[test]
fn a_script_made_from_the_media_template_declares_every_media_event() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    act(&mut p, json!({ "import_script": { "file": "", "template": "media" } })).unwrap();
    act(&mut p, json!({ "import_script": { "file": "", "template": "media" } })).unwrap();
    let scripts = p.doc["scene"]["scripts"].clone();
    assert_eq!(scripts[0]["path"], "scripts/media.js");
    assert_eq!(scripts[1]["path"], "scripts/media-2.js");
    let events = ["mediaStatus", "mediaPlayback", "mediaProperties", "mediaThumbnail", "mediaTimeline"];
    assert_eq!(scripts[0]["events"], json!(events));
    assert_eq!(scripts[0]["uses"], json!(["media"]));
    assert!(p.dir.join("scripts/media.js").is_file());
    let problems = wprs_editor::package::problems(&p.dir, &p.doc);
    assert!(problems.iter().all(|c| c.warning), "{problems:?}");
    act(&mut p, json!({ "import_script": { "file": "plain.js", "template": "empty" } })).unwrap();
    assert_eq!(p.doc["scene"]["scripts"][2]["path"], "scripts/plain.js");
    let taken = act(&mut p, json!({ "import_script": { "file": "plain.js", "template": "empty" } }));
    assert!(taken.unwrap_err().to_string().contains("already exists"));
    let unknown = act(&mut p, json!({ "import_script": { "file": "x.js", "template": "nope" } }));
    assert!(unknown.unwrap_err().to_string().contains("no script template"));
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert_eq!(p.doc["scene"]["scripts"].as_array().unwrap().len(), 2);
}

#[test]
fn own_clock_lanes_take_a_strip_of_frames_of_their_video_once() {
    let mut doc = starter("empty", "org.example.clocks", "Clocks", "en").unwrap();
    doc["scene"]["assets"]["clip"] = json!({ "path": "clip.webm" });
    doc["scene"]["layers"] =
        json!([{ "id": "film", "type": "video", "video": "clip", "playback": { "own_clock": true } }]);
    let dir = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("XDG_CACHE_HOME", dir.path().join("cache")) };
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/magenta-64x32.webm");
    std::fs::copy(fixture, dir.path().join("clip.webm")).unwrap();
    let strip = &wprs_editor::clocks::clocks(dir.path(), &doc, 0.0, 4)["clocks"][0]["strip"];
    let frames = strip.as_array().unwrap();
    if frames.is_empty() {
        return assert!(std::env::var_os("WPRS_REQUIRE_VIDEO").is_none(), "no frames: {strip}");
    }
    assert_eq!(frames.len(), 4, "{strip}");
    let first = frames[0]["file"].as_str().unwrap();
    assert!(
        first.starts_with(dir.path().join("cache").to_str().unwrap())
            && image::open(first).unwrap().height() == 48
    );
    assert_eq!(&wprs_editor::clocks::clocks(dir.path(), &doc, 0.0, 4)["clocks"][0]["strip"], strip, "kept");
}
