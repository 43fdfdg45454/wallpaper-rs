use super::{act, edit, project, system};
use serde_json::json;

#[test]
fn an_emitter_bursts_pauses_waits_follows_and_listens() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let values = json!({"burst": {"count": 10, "every": 2}, "periodic": {"on": [1, 2], "off": [3, 4]},
        "duration": 5, "delay": 1, "follow": "cursor", "audio_gate": true, "inherit": 0.5});
    edit(&mut p, "emit", json!({"emitter": 0, "values": values})).unwrap();
    let e = &system(&p)["emitters"][0];
    assert_eq!(
        (&e["burst"]["count"], &e["follow"], &e["audio_gate"]),
        (&json!(10), &json!("cursor"), &json!(true))
    );
    edit(&mut p, "emit", json!({"emitter": 0, "clear": ["burst"], "values": {"audio_gate": false}})).unwrap();
    let e = &system(&p)["emitters"][0];
    assert!(e.get("burst").is_none() && e.get("audio_gate").is_none() && e["delay"] == json!(1));
    assert!(edit(&mut p, "emit", json!({"emitter": 0, "values": {"wobble": 1}})).is_err());
    assert!(edit(&mut p, "emit", json!({"emitter": 0, "values": {"burst": {"count": "many"}}})).is_err());
    assert!(edit(&mut p, "emit", json!({"emitter": 9, "values": {"delay": 1}})).is_err());
    assert!(edit(&mut p, "emit", json!({"emitter": 0, "values": {"delay": -1}})).is_err());
}

#[test]
fn a_force_acts_inside_a_window_of_the_life_and_a_system_locks_values() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    edit(&mut p, "force", json!({"force": {"gravity": [0, 40]}})).unwrap();
    let last = system(&p)["forces"].as_array().unwrap().len() - 1;
    edit(&mut p, "window", json!({"index": last, "window": [0, 0.1, 0.8, 1]})).unwrap();
    assert_eq!(system(&p)["forces"][last]["window"], json!([0.0, 0.1, 0.8, 1.0]));
    assert!(edit(&mut p, "window", json!({"index": last, "window": [0.5, 0.1, 0.8, 1]})).is_err());
    assert!(edit(&mut p, "window", json!({"index": 40, "window": [0, 0, 1, 1]})).is_err());
    edit(&mut p, "force", json!({"index": last, "force": {"gravity": [0, 200]}})).unwrap();
    let kept = &system(&p)["forces"][last];
    assert_eq!((&kept["gravity"], &kept["window"]), (&json!([0, 200]), &json!([0.0, 0.1, 0.8, 1.0])));
    edit(&mut p, "window", json!({"index": last})).unwrap();
    assert!(system(&p)["forces"][last].get("window").is_none());
    edit(&mut p, "settings", json!({"lock": ["alpha", "size"]})).unwrap();
    assert_eq!(system(&p)["lock"], json!(["alpha", "size"]));
    assert!(edit(&mut p, "settings", json!({"lock": ["wobble"]})).is_err());
    edit(&mut p, "settings", json!({"lock": ["none"]})).unwrap();
    assert!(system(&p).get("lock").is_none());
    edit(&mut p, "render", json!({"frames": "life", "cycles": 3, "order": "oldest"})).unwrap();
    assert_eq!(
        (&system(&p)["render"]["cycles"], &system(&p)["render"]["order"]),
        (&json!(3), &json!("oldest"))
    );
    assert!(edit(&mut p, "render", json!({"order": "sideways"})).is_err());
    edit(&mut p, "render", json!({"cycles": 0, "order": "none"})).unwrap();
    assert!(system(&p)["render"].get("order").is_none() && system(&p)["render"].get("cycles").is_none());
}

#[test]
fn starting_colors_come_from_hues_a_palette_or_a_gradient() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let hues = json!({"hsv": {"hue": [180, 240], "steps": 4, "sat": [0.5, 1], "val": [0.8, 1]}});
    edit(&mut p, "start", json!({ "ranges": hues })).unwrap();
    assert_eq!(system(&p)["init"]["hsv"]["steps"], json!(4));
    let palette = json!({"palette": {"colors": ["#ff0000", "#00ff00"], "noise": [0.1, 0.1, 0.1]}});
    edit(&mut p, "start", json!({ "ranges": palette })).unwrap();
    let init = &system(&p)["init"];
    assert!(init.get("hsv").is_none() && init["palette"]["colors"][1] == json!("#00ff00"));
    edit(&mut p, "start", json!({"ranges": {"gradient": [[0, "#000000"], [1, "#ffffff"]]}})).unwrap();
    assert!(system(&p)["init"].get("palette").is_none());
    let both = json!({"ranges": {"hsv": {"hue": [0, 10]}, "gradient": [[0, "#000000"]]}});
    assert!(edit(&mut p, "start", both).unwrap_err().to_string().contains("one of them"));
    assert!(edit(&mut p, "start", json!({"ranges": {"hsv": {"hue": "red"}}})).is_err());
    assert!(
        edit(&mut p, "start", json!({"ranges": {"palette": {"colors": [], "noise": [2, 0, 0]}}})).is_err()
    );
    edit(&mut p, "start", json!({"clear": ["hsv", "palette", "gradient"]})).unwrap();
    assert!(["hsv", "palette", "gradient"].iter().all(|k| system(&p)["init"].get(*k).is_none()));
}

#[test]
fn a_system_is_renamed_with_the_layers_and_systems_that_use_it() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    act(&mut p, json!({"particles": {"create": {"system": "sparks", "parent": "flakes"}}})).unwrap();
    act(&mut p, json!({"particles": {"rename": {"system": "sparks", "to": "embers"}}})).unwrap();
    assert_eq!(system(&p)["children"][0]["system"], "embers");
    edit(&mut p, "rename", json!({"to": "snow"})).unwrap();
    let scene = &p.doc["scene"];
    assert!(scene["particles"].get("flakes").is_none() && scene["particles"]["snow"]["children"].is_array());
    let layer = scene["layers"].as_array().unwrap().iter().find(|l| l["type"] == "particles").unwrap();
    assert_eq!(layer["system"], "snow");
    assert!(act(&mut p, json!({"particles": {"rename": {"system": "snow", "to": "embers"}}})).is_err());
    assert!(act(&mut p, json!({"particles": {"rename": {"system": "snow", "to": "Bad Id"}}})).is_err());
}
