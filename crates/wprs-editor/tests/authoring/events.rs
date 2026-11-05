use super::{act, at, project, refused};
use serde_json::{Value, json};
use wprs_editor::actions::{Action, TimelineEdit, TimelineEvent};

fn line(id: &str, more: Value) -> Value {
    let mut edit = json!({ "id": id });
    edit.as_object_mut().unwrap().extend(more.as_object().unwrap().clone());
    json!({ "timeline": edit })
}

#[test]
fn the_events_of_a_timeline_are_added_moved_and_removed_within_its_length() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    act(&mut p, line("intro", json!({"add": true, "duration": 4}))).unwrap();
    let add = |t: f64, name: &str| json!({"add_events": [{"t": t, "name": name}]});
    act(&mut p, line("intro", add(3.0, "boom"))).unwrap();
    act(&mut p, line("intro", add(1.25, "spark"))).unwrap();
    act(&mut p, line("intro", add(2.0, "boom"))).unwrap();
    let events = at(&p, "/scene/timelines/0/events");
    assert_eq!(
        events,
        json!([{"t": 1.25, "name": "spark"}, {"t": 2.0, "name": "boom"}, {"t": 3.0, "name": "boom"}])
    );
    for (t, name, words) in [
        (5.0, "late", "end of the timeline"),
        (-1.0, "early", "end of the timeline"),
        (1.0, "Big Bang", "event name"),
        (3.0, "boom", "already at"),
    ] {
        let error = refused(&mut p, line("intro", add(t, name)));
        assert!(error.contains(words), "{error}");
    }
    let nan = TimelineEvent { t: f64::NAN, name: "nan".into() };
    let edit = TimelineEdit { id: "intro".into(), add_events: vec![nan], ..TimelineEdit::default() };
    let before = p.doc.clone();
    assert!(p.act(&Action::Timeline(edit)).unwrap_err().to_string().contains("end of the timeline"));
    assert_eq!(p.doc, before);
    let moved = json!({"drop_events": ["spark@1.25"], "add_events": [{"t": 0.5, "name": "spark"}]});
    act(&mut p, line("intro", moved)).unwrap();
    assert_eq!(at(&p, "/scene/timelines/0/events/0"), json!({"t": 0.5, "name": "spark"}));
    let missing = refused(&mut p, line("intro", json!({"drop_events": ["spark@9"]})));
    assert!(missing.contains("no event spark@9"), "{missing}");
    let short = refused(&mut p, line("intro", json!({"duration": 2})));
    assert!(short.contains("events"), "{short}");
    act(&mut p, line("intro", json!({"drop_events": ["boom"]}))).unwrap();
    assert_eq!(at(&p, "/scene/timelines/0/events"), json!([{"t": 0.5, "name": "spark"}]));
    act(&mut p, line("intro", json!({"clear_events": true}))).unwrap();
    assert_eq!(at(&p, "/scene/timelines/0/events"), Value::Null);
    let many: Vec<Value> =
        (0..65).map(|i| json!({"t": 0.01 * f64::from(i), "name": format!("e{i}")})).collect();
    let crowded = refused(&mut p, line("intro", json!({ "add_events": many })));
    assert!(crowded.contains("at most 64"), "{crowded}");
}
