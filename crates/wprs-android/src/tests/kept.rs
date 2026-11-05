use super::*;
use crate::keeping::Keeping;

fn stored() -> std::sync::Arc<pack::Pack> {
    let source = tempfile::tempdir().unwrap();
    let scripts = json!([{ "path": "keep.js", "events": ["init"], "uses": ["storage"] }]);
    let text = json!({ "schema": wprs_scene::SCHEMA, "canvas": { "size": [32, 16] },
        "layers": [{ "id": "dot", "type": "solid", "color": "#ffffff", "size": [4, 4] }], "scripts": scripts });
    fs::write(source.path().join("scene.json"), text.to_string()).unwrap();
    fs::write(source.path().join("keep.js"), "export function init() {}\n").unwrap();
    let manifest = json!({ "schema": wprs_format::SCHEMA, "id": "org.example.keeps", "version": "1.0.0",
        "title": "Keeps", "license": "CC0-1.0", "kind": "scene", "entry": "scene.json",
        "content": { "rating": "general" }, "photosensitive": false, "origin": { "kind": "original" } });
    fs::write(source.path().join("manifest.json"), manifest.to_string()).unwrap();
    let wpak = source.path().join("keeps.wpak");
    wprs_format::build(source.path(), &wpak, &wprs_format::Limits::default()).unwrap();
    let uid = store::store(&mut File::open(&wpak).unwrap(), "keeps.wpak").unwrap().0;
    pack::load(&library::with(|lib| Ok(lib.item(&uid)?.path)).unwrap()).unwrap()
}

fn scopes(keeping: &mut Keeping, pack: &pack::Pack) -> Value {
    keeping.list((&pack.properties, &pack.scene), &Default::default(), 0.0);
    serde_json::from_str(keeping.storage().1).unwrap()
}

pub(super) fn home_and_lock_keep_their_own_storage() {
    let pack = stored();
    let (mut home, mut lock) = (Keeping::new(&pack, false), Keeping::new(&pack, true));
    let wrote = |stored: Value| wprs_scene::ScriptOutputs { storage: Some(stored), ..Default::default() };
    home.heard(&mut wrote(json!({ "set": { "output": { "seen": "home" }, "package": { "count": 2 } } })));
    lock.heard(&mut wrote(json!({ "set": { "output": { "seen": "lock" } } })));
    let (at_home, at_lock) = (scopes(&mut home, &pack), scopes(&mut lock, &pack));
    assert_eq!((&at_home["output"]["seen"], &at_lock["output"]["seen"]), (&json!("home"), &json!("lock")));
    assert_eq!((&at_home["package"]["count"], &at_lock["package"]["count"]), (&json!(2), &json!(2)));
    let again = scopes(&mut Keeping::new(&pack, true), &pack);
    assert_eq!(again["output"]["seen"], json!("lock"), "{again}");
}
