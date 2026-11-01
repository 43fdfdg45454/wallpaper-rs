use super::*;
use wprs_script::{Limits, Script};

const STORE: &str = r#"import { scene, storage } from "wprs";
    export function update({ time }) {
        const said = [];
        const step = (f) => { try { said.push(f()); } catch (e) { said.push(e.message); } };
        step(() => storage.get("visits") ?? 0);
        step(() => storage.get("seat", "output") ?? "none");
        if (time > 0) {
            step(() => (storage.set("visits", (storage.get("visits") ?? 0) + 1), "set"));
            step(() => (storage.set("seat", { row: 2 }, "output"), "set"));
            step(() => storage.set("big", "x".repeat(70000)));
            step(() => storage.set("f", () => 1));
            step(() => storage.get("x", "screen"));
        }
        scene.layer("info").text = said.join("|");
    }"#;

const LIST: &str = r#"import { scene, folder } from "wprs";
    const said = [];
    let album = null;
    try { album = folder("album"); } catch (e) { said.push(e.message); }
    if (album) for (const e of ["added", "removed", "changed"]) album.on(e, ({ files }) => said.push(`${e}:${JSON.stringify(files)}`));
    export function update() {
        if (album && album.files.length) {
            try { album.show("photo", album.files[0]); album.show("photo", "nope"); } catch (e) { said.push(e.message); }
            said.push(album.name(album.files[0]));
        }
        scene.layer("info").text = said.join("|");
    }"#;

fn scene(uses: serde_json::Value) -> serde_json::Value {
    json!({
        "scripts": [{ "path": "main.js", "uses": uses }],
        "layers": [{ "id": "photo", "type": "solid", "color": "#ffffff" }, { "id": "info", "type": "text", "text": "", "size": 10 }]
    })
}

fn with(uses: serde_json::Value, source: &str, setup: impl FnOnce(&mut wprs_script::Setup)) -> Script {
    let mut made = common::setup(scene(uses), &[("main.js", source)]);
    setup(&mut made);
    Script::load(&made, Limits::default()).unwrap()
}

#[test]
fn scripts_keep_data_per_package_and_output_within_bounds() {
    let mut script = with(json!(["storage"]), STORE, |s| s.storage = json!({ "package": { "visits": 4 } }));
    let first = at(&mut script, 0.0).unwrap();
    assert_eq!((text(&first, "info"), first.storage.clone()), ("4|none".into(), None));
    let out = at(&mut script, 0.1).unwrap();
    let said = text(&out, "info");
    assert!(said.starts_with("4|none|set|set|storage keeps at most 64 KiB in each scope|storage keeps only JSON values|storage takes the scope package or output"), "{said}");
    assert_eq!(out.storage, Some(json!({ "package": { "visits": 5 }, "output": { "seat": { "row": 2 } } })));
    assert_eq!(at(&mut script, 0.2).unwrap().storage.map(|s| s["package"]["visits"].clone()), Some(json!(6)));
    let mut plain = with(json!(["timers"]), STORE, |_| {});
    let said = text(&at(&mut plain, 0.1).unwrap(), "info");
    assert!(said.starts_with("main.js uses storage without declaring it in scene.json"), "{said}");
}

#[test]
fn scripts_list_a_chosen_folder_and_pick_what_a_layer_shows() {
    let mut script = with(json!(["folders"]), LIST, |s| s.folders = vec!["album".into()]);
    let listed = r#"{"album":[["a1","k1","sea.png"],["b1","k2","sky.png"]]}"#;
    let out = script.update(&Frame { folders: listed, ..Frame::at(0.0) }).unwrap();
    assert_eq!(out.layers["photo"].file.as_deref(), Some("wpak-user://album/a1"));
    assert_eq!(text(&out, "info"), r#"added:["a1","b1"]|nope is not a file of album|sea.png"#);
    let again = r#"{"album":[["a2","k1","sea.png"],["c1","k3",""]]}"#;
    let out = script.update(&Frame { folders: again, ..Frame::at(0.1) }).unwrap();
    let said = text(&out, "info");
    assert!(
        said.contains(r#"added:["c1"]"#)
            && said.contains(r#"removed:["b1"]"#)
            && said.contains(r#"changed:[["a1","a2"]]"#),
        "{said}"
    );
    let mut refused = with(json!(["folders"]), LIST, |_| {});
    assert_eq!(
        text(&at(&mut refused, 0.0).unwrap(), "info"),
        "album is not a folder property that lists its files"
    );
    let mut plain = with(json!(["timers"]), LIST, |s| s.folders = vec!["album".into()]);
    assert_eq!(
        text(&at(&mut plain, 0.0).unwrap(), "info"),
        "main.js uses folders without declaring it in scene.json"
    );
}
