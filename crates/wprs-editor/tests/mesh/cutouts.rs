use super::kit::{act, parts};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use wprs_editor::Project;

fn edit(extra: Value) -> Value {
    let mut e = json!({ "layer": "body" });
    e.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    json!({ "parts": e })
}

fn shown(p: &Project) -> Value {
    p.doc["scene"]["layers"][0]["parts"].clone()
}

#[test]
fn parts_are_ordered_hidden_and_shown_only_over_others() {
    let root = tempfile::tempdir().unwrap();
    let mut p = parts(&root);
    act(
        &mut p,
        edit(json!({ "order": { "chest": -2, "legs": 1.4 }, "hide": ["legs"], "clip": { "chest": "legs" } })),
    )
    .unwrap();
    assert_eq!(
        shown(&p),
        json!({ "chest": { "order": -2.0, "clip_to": "legs" }, "legs": { "order": 1.0, "visible": false } })
    );
    act(&mut p, edit(json!({ "show": ["legs"], "unclip": ["chest"] }))).unwrap();
    assert_eq!(shown(&p), json!({ "chest": { "order": -2.0 }, "legs": { "order": 1.0 } }));
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert_eq!(shown(&p)["chest"]["clip_to"], json!("legs"));
    for bad in [
        json!({ "hide": ["arm"] }),
        json!({ "clip": { "chest": "chest" } }),
        json!({ "clip": { "legs": "chest" } }),
        json!({ "clip": { "chest": "nothing" } }),
        json!({ "order": { "legs": 2000 } }),
        json!({ "hide": ["legs"], "show": ["legs"] }),
        json!({}),
    ] {
        assert!(act(&mut p, edit(bad.clone())).is_err(), "{bad}");
    }
    act(&mut p, json!({ "redo": {} })).unwrap();
    act(&mut p, edit(json!({ "order": { "chest": 0, "legs": 0 } }))).unwrap();
    assert_eq!(shown(&p), json!({ "chest": { "order": 0.0 }, "legs": { "order": 0.0 } }));
    let rims = wprs_editor::rigs::outline(&p.dir, &p.doc, "body");
    assert_eq!(rims.as_object().unwrap().keys().collect::<Vec<_>>(), ["chest", "legs"]);
    for (part, edges) in rims.as_object().unwrap() {
        let mut ends: BTreeMap<String, u32> = BTreeMap::new();
        for e in edges.as_array().unwrap() {
            e.as_array().unwrap().iter().for_each(|v| *ends.entry(v.to_string()).or_default() += 1);
        }
        assert!(ends.len() >= 3 && ends.values().all(|n| n % 2 == 0), "{part}: {edges}");
    }
}
