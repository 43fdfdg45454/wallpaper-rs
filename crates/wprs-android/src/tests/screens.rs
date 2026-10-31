use super::{ask, open};
use crate::choice::{self, Target};
use crate::{settings, store};
use serde_json::{Value, json};

fn on(lock: bool) -> String {
    choice::shown_on(false, lock).map(|shown| shown.uid).unwrap_or_default()
}

fn set(choice: Value, target: Target) {
    assert_eq!(ask(json!({ "op": "pick", "choice": choice })), Value::Null);
    choice::commit(true, target).unwrap();
}

pub(super) fn home_and_lock() {
    let green = store::store(&mut open("green-64x32.png"), "green.png").unwrap().0;
    let bands = store::store(&mut open("bands-640x180.png"), "bands.png").unwrap().0;
    set(json!({ "item": green }), Target::Both);
    assert_eq!((on(false), on(true)), (green.clone(), green.clone()));
    set(json!({ "item": bands }), Target::Lock);
    assert_eq!((on(false), on(true)), (green.clone(), bands.clone()));
    set(json!({ "item": bands }), Target::Home);
    set(json!({ "item": green }), Target::Home);
    assert_eq!((on(false), on(true)), (green.clone(), bands.clone()), "the lock keeps its own choice");
    set(json!({ "item": bands }), Target::Both);
    assert_eq!((on(false), on(true)), (bands.clone(), bands.clone()));
    set(json!({ "item": green }), Target::Home);
    assert_eq!((on(false), on(true)), (green.clone(), bands.clone()), "home alone leaves the lock as it was");
    let list = ask(json!({ "op": "create", "name": "Both" })).as_str().unwrap().to_owned();
    ask(json!({ "op": "add", "playlist": list, "items": [green, bands] }));
    set(json!({ "playlist": list }), Target::Lock);
    let listed = ask(json!({ "op": "playlists" }));
    let row = listed.as_array().unwrap().iter().find(|p| p["playlist"]["uid"] == json!(list)).unwrap();
    assert_eq!((&row["home"], &row["lock"], &row["current"]), (&json!(false), &json!(true), &json!(true)));
    assert_eq!((on(false), on(true)), (green.clone(), green.clone()));
    assert!(!choice::advance(true, true).unwrap() && choice::colors(false, true).unwrap().is_some());
    assert_eq!(ask(json!({ "op": "delete", "playlist": list })), Value::Null);
    assert_eq!(on(true), green, "a deleted list leaves the lock showing the home screen");
    set(json!({ "item": bands }), Target::Lock);
    assert_eq!(ask(json!({ "op": "remove", "uids": [bands] })), Value::Null);
    assert_eq!((on(false), on(true)), (green.clone(), green.clone()));
    assert!(settings::config().outputs.get(choice::LOCK).is_none_or(|lock| lock.image.is_none()));
}

pub(super) fn ruled_lists() {
    let morning = ask(json!({ "op": "create", "name": "Morning" })).as_str().unwrap().to_owned();
    let info = json!({ "uid": morning, "name": "Mañana", "query": "kind:image",
        "rules": { "interval": "1h", "at": ["07:30", "18:00"], "days": ["Mon", "Sat"] } });
    assert_eq!(ask(json!({ "op": "save", "playlist": info })), Value::Null);
    let listed = ask(json!({ "op": "playlists" }));
    let row = listed.as_array().unwrap().iter().find(|p| p["playlist"]["uid"] == json!(morning)).unwrap();
    let rules = &row["playlist"]["rules"];
    assert_eq!(
        (&row["playlist"]["name"], &rules["at"], &rules["days"]),
        (&json!("Mañana"), &json!(["07:30", "18:00"]), &json!(["Mon", "Sat"]))
    );
    let images = ask(json!({ "op": "count", "query": "kind:image" }));
    assert_eq!(row["count"], images, "a smart list holds what its search finds");
    assert!(ask(json!({ "op": "count", "query": "\"open" }))["error"].is_string());
    let bad = json!({ "uid": morning, "name": "Mañana", "query": null, "rules": { "at": ["25:00"] } });
    assert!(ask(json!({ "op": "save", "playlist": bad }))["error"].is_string());
    assert_eq!(ask(json!({ "op": "delete", "playlist": morning })), Value::Null);
}
