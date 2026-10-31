use super::*;

const TAP: &str = r#"import { scene } from "wprs";
let taps = 0;
export function init() {
    scene.layer("button").on("click", () => { taps += 1; scene.layer("note").text = `taps ${taps}`; });
    scene.layer("note").text = "ready";
}
"#;

fn built(name: &str, script: &str) -> String {
    let source = tempfile::tempdir().unwrap();
    let light = json!({ "timeline": "light", "keys": [[0, 0], [0.1, 1]] });
    let layers = json!([
        { "id": "button", "type": "solid", "color": "#2040c0", "size": [32, 16], "solid": true, "transform": { "position": [16, 16] } },
        { "id": "lamp", "type": "solid", "color": "#ffffff", "size": [8, 8], "opacity": light, "transform": { "position": [56, 16] } },
        { "id": "note", "type": "text", "text": "", "size": 8, "color": "#ffffff", "font": "sans-serif" }]);
    let timelines = json!([{ "id": "light", "duration": 0.1, "autoplay": false, "trigger": "click:button" }]);
    let scripts = json!([{ "path": "tap.js", "events": ["init", "click"], "uses": ["layers", "clicks"] }]);
    let text = json!({ "schema": wprs_scene::SCHEMA, "canvas": { "size": [64, 32] }, "layers": layers,
        "timelines": timelines, "scripts": scripts });
    fs::write(source.path().join("scene.json"), text.to_string()).unwrap();
    fs::write(source.path().join("tap.js"), script).unwrap();
    let manifest = json!({ "schema": wprs_format::SCHEMA, "id": format!("org.example.{name}"), "version": "1.0.0",
        "title": name, "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "content": { "rating": "general" },
        "photosensitive": false, "origin": { "kind": "original" }, "inputs": ["pointer", "clicks"] });
    fs::write(source.path().join("manifest.json"), manifest.to_string()).unwrap();
    let wpak = source.path().join(format!("{name}.wpak"));
    wprs_format::build(source.path(), &wpak, &wprs_format::Limits::default()).unwrap();
    store::store(&mut File::open(&wpak).unwrap(), &format!("{name}.wpak")).unwrap().0
}

pub(super) fn scripted() {
    let uid = built("taps", TAP);
    let form = ask(json!({ "op": "scene", "uid": uid, "languages": [] }));
    assert_eq!((&form["uses"]["taps"], &form["granted"]["taps"]), (&json!(true), &json!(false)), "{form}");
    let path = library::with(|lib| Ok(lib.item(&uid)?.path)).unwrap();
    let power =
        power::Power { charging: true, saver: false, low: false, thermal: 0, headroom: 0.0, still: false };
    let Some(r) = renderer() else { return };
    let mut stage = Stage::new(uid.clone(), pack::load(&path).unwrap());
    let mut clock = 0;
    let mut until = |stage: &mut Stage, done: &dyn Fn(&Stage) -> bool| {
        let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            clock += 50_000_000;
            stage.render(r, ((64, 32), clock, 60), (power, wprs_scene::Tier::T3), &[], true).unwrap();
            if done(stage) || std::time::Instant::now() >= end {
                return done(stage);
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    };
    let note = |stage: &Stage| stage.runtime().read("note").and_then(|n| n.text).unwrap_or_default();
    let lit = |stage: &Stage| stage.runtime().read("lamp").map_or(0.0, |l| l.opacity);
    assert!(until(&mut stage, &|s| note(s) == "ready"), "the scene script did not run on the phone");
    assert!(!stage.touch([0.5, 0.75], true), "taps reached a scene without the grant");
    assert_eq!(ask(json!({ "op": "grant", "uid": uid, "input": "taps", "on": true })), Value::Null);
    let mut stage = Stage::new(uid.clone(), pack::load(&path).unwrap());
    assert!(until(&mut stage, &|s| note(s) == "ready"));
    assert!(stage.wants()[1], "a scene with taps allowed takes touch events");
    assert!(stage.touch([0.9, 0.9], true) && stage.touch([0.9, 0.9], false));
    assert!(!until(&mut stage, &|s| note(s) != "ready" || lit(s) > 0.0), "a tap beside the button counted");
    assert!(stage.touch([0.5, 0.75], true) && stage.touch([0.5, 0.75], false));
    assert!(until(&mut stage, &|s| note(s) == "taps 1"), "the tap did not reach the script");
    assert!(until(&mut stage, &|s| lit(s) > 0.9), "the tap did not start the click: timeline");
    assert!(stage.touch([0.5, 0.75], true) && until(&mut stage, &|_| true));
    assert!(stage.touch([0.25, 0.5], true) && until(&mut stage, &|_| true));
    assert_eq!(stage.runtime().pointer().pos, None, "taps alone let the scene follow the finger");
    assert!(stage.touch([0.25, 0.5], false));
    assert!(until(&mut stage, &|s| note(s) == "taps 2"), "a tap held over two frames did not click");
    assert_eq!(crate::acting::scripts(), "");
    let looping = built("loop", "export function update() { for (;;) {} }\n");
    let path = library::with(|lib| Ok(lib.item(&looping)?.path)).unwrap();
    let mut stuck = Stage::new(looping.clone(), pack::load(&path).unwrap());
    assert!(
        until(&mut stuck, &|_| crate::acting::scripts() == "off"),
        "a script that never returns kept running"
    );
    crate::stage::report::finish();
    for uid in [uid, looping] {
        assert_eq!(ask(json!({ "op": "remove", "uids": [uid] })), Value::Null);
    }
}
