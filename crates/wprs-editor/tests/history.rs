use serde_json::{Value, json};
use wprs_editor::History;
use wprs_editor::ops::{Op, apply, set};

fn doc() -> Value {
    json!({"scene": {"layers": [{"id": "a"}, {"id": "b"}, {"id": "c"}], "canvas": {"size": [1, 1]}, "a~/b": 1}})
}

fn replace(path: &str, value: Value) -> Op {
    Op::Replace { path: path.into(), value }
}

#[test]
fn every_operation_has_an_inverse() {
    let ops = vec![
        Op::Add { path: "/scene/layers/-".into(), value: json!({"id": "d"}) },
        Op::Add { path: "/scene/layers/0".into(), value: json!({"id": "z"}) },
        Op::Remove { path: "/scene/layers/2".into() },
        replace("/scene/canvas/size", json!([2, 2])),
        Op::Move { from: "/scene/layers/0".into(), path: "/scene/layers/3".into() },
        Op::Add { path: "/scene/canvas/fit".into(), value: json!("fit") },
        Op::Remove { path: "/scene/a~0~1b".into() },
    ];
    let mut edited = doc();
    let undo = apply(&mut edited, &ops).unwrap();
    let ids: Vec<&str> =
        edited["scene"]["layers"].as_array().unwrap().iter().map(|l| l["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["a", "c", "d", "z"]);
    assert_eq!((&edited["scene"]["canvas"]["fit"], edited["scene"].get("a~/b")), (&json!("fit"), None));
    let redo = apply(&mut edited, &undo).unwrap();
    assert_eq!(edited, doc());
    apply(&mut edited, &redo).unwrap();
    assert_eq!(edited["scene"]["layers"][3]["id"], "z");
    let text = serde_json::to_string(&ops[4]).unwrap();
    assert_eq!(text, r#"{"op":"move","from":"/scene/layers/0","path":"/scene/layers/3"}"#);
}

#[test]
fn a_failing_operation_changes_nothing() {
    let mut edited = doc();
    let ops =
        vec![replace("/scene/canvas/size", json!([5, 5])), Op::Remove { path: "/scene/layers/7".into() }];
    assert!(apply(&mut edited, &ops).is_err());
    let into_itself = Op::Move { from: "/scene/layers".into(), path: "/scene/layers/0/x".into() };
    let taken = Op::Move { from: "/scene/layers".into(), path: "/scene/canvas".into() };
    assert!(apply(&mut edited, &[into_itself]).is_err() && apply(&mut edited, &[taken]).is_err());
    assert!(apply(&mut edited, &[Op::Add { path: "/scene/layers/01".into(), value: json!(1) }]).is_err());
    assert_eq!(edited, doc());
    let made = set(&edited, "/scene/layers/1/transform/position", json!([4, 5]));
    assert_eq!(made.len(), 2);
    apply(&mut edited, &made).unwrap();
    assert_eq!(edited["scene"]["layers"][1]["transform"]["position"], json!([4, 5]));
}

#[test]
fn continuous_drags_merge_into_one_step() {
    let (mut edited, mut history) = (doc(), History::default());
    history.mark_saved();
    for (i, at) in [0, 100, 450, 900].into_iter().enumerate() {
        history
            .record(&mut edited, "set", vec![replace("/scene/canvas/size", json!([i, i]))], at, true)
            .unwrap();
    }
    assert_eq!(history.done.len(), 1, "every move came within 500 ms of the last");
    history
        .record(&mut edited, "set", vec![replace("/scene/canvas/size", json!([9, 9]))], 2000, true)
        .unwrap();
    history.record(&mut edited, "set", vec![replace("/scene/layers/0/id", json!("q"))], 2100, true).unwrap();
    history.record(&mut edited, "hide", vec![replace("/scene/layers/0/id", json!("r"))], 2150, true).unwrap();
    assert_eq!(history.labels().0, ["set", "set", "set", "hide"]);
    assert!(history.dirty());
    for _ in 0..4 {
        history.undo(&mut edited).unwrap().unwrap();
    }
    assert_eq!((edited.clone(), history.dirty()), (doc(), false));
    assert!(history.undo(&mut edited).unwrap().is_none());
    history.redo(&mut edited).unwrap();
    assert_eq!(edited["scene"]["canvas"]["size"], json!([3, 3]));
    history
        .record(&mut edited, "add", vec![Op::Add { path: "/scene/x".into(), value: json!(1) }], 5000, true)
        .unwrap();
    assert!(history.undone.is_empty() && history.redo(&mut edited).unwrap().is_none());
}

#[test]
fn a_drag_that_creates_its_value_merges_and_moves_never_do() {
    let (mut edited, mut history) = (doc(), History::default());
    let pointer = "/scene/layers/1/transform/position";
    for (i, at) in [0, 100, 200].into_iter().enumerate() {
        let ops = set(&edited, pointer, json!([i, i]));
        history.record(&mut edited, "set", ops, at, true).unwrap();
    }
    assert_eq!(history.done.len(), 1);
    history.undo(&mut edited).unwrap();
    assert_eq!(edited, doc());
    history.redo(&mut edited).unwrap();
    assert_eq!(edited.pointer(pointer), Some(&json!([2, 2])));
    let step = |from: &str| vec![Op::Move { from: from.into(), path: "/scene/layers/2".into() }];
    history.record(&mut edited, "move", step("/scene/layers/0"), 300, true).unwrap();
    history.record(&mut edited, "move", step("/scene/layers/0"), 350, true).unwrap();
    assert_eq!(history.done.len(), 3);
    history.undo(&mut edited).unwrap();
    history.undo(&mut edited).unwrap();
    history.undo(&mut edited).unwrap();
    assert_eq!(edited, doc());
}

#[test]
fn the_history_keeps_its_bounds() {
    let (mut edited, mut history) = (doc(), History { limits: Some((3, 1 << 20)), ..History::default() });
    for i in 0..5 {
        history
            .record(&mut edited, "set", vec![replace("/scene/canvas/size", json!([i, i]))], i * 1000, true)
            .unwrap();
        if i == 2 {
            history.mark_saved();
        }
    }
    assert_eq!((history.done.len(), history.saved), (3, Some(1)));
    while history.undo(&mut edited).unwrap().is_some() {}
    assert_eq!(edited["scene"]["canvas"]["size"], json!([1, 1]));
    history.limits = Some((100, 64));
    let big = json!("x".repeat(40));
    for i in 0..4 {
        history
            .record(
                &mut edited,
                "set",
                vec![replace("/scene/layers/0/id", big.clone())],
                10_000 + i * 1000,
                true,
            )
            .unwrap();
    }
    assert!(history.done.len() == 1 && history.done[0].size < 128);
}

#[test]
fn only_gestures_merge() {
    let (mut edited, mut history) = (doc(), History::default());
    let size = |n: i32| vec![replace("/scene/canvas/size", json!([n, n]))];
    history.record(&mut edited, "set", size(1), 0, false).unwrap();
    history.record(&mut edited, "set", size(2), 10, false).unwrap();
    history.record(&mut edited, "set", size(3), 20, true).unwrap();
    history.record(&mut edited, "set", size(4), 30, true).unwrap();
    assert_eq!(history.done.len(), 3);
    history.undo(&mut edited).unwrap();
    assert_eq!(edited["scene"]["canvas"]["size"], json!([2, 2]));
}

#[test]
fn steps_layers_refusals_and_problems_keep_ids_and_speak_spanish() {
    let es = wprs_core::message::tr_in(&["es_ES.UTF-8".into()]);
    let step = wprs_editor::history::step("add_layer");
    assert_eq!((step.id, step.told(&es)), ("editor.step.add_layer", "Añadir una capa".into()));
    assert_eq!(wprs_editor::history::step("mystery").told(&es), "mystery");
    let sky = wprs_editor::starters::label("sky").unwrap();
    assert_eq!((sky.to_string(), sky.told(&es)), ("Sky".into(), "Cielo".into()));
    assert!(wprs_editor::starters::label("mine").is_none());
    let mut doc = wprs_editor::starters::starter("parallax", "org.example.es", "Alba", "es").unwrap();
    let action = serde_json::from_value(json!({"remove_layer": {"id": "nope"}})).unwrap();
    let refused = wprs_editor::actions::plan(&doc, &action).unwrap_err();
    assert_eq!(refused.to_string(), "there is no layer \"nope\"");
    assert_eq!(wprs_editor::told_with(&refused, &es), "no hay ninguna capa «nope»");
    doc["scene"]["layers"][0]["id"] = json!("Bad sky");
    let found = wprs_editor::check::check(&doc);
    let bad = found.iter().find(|p| p.said.id == "scene.id").expect("an id problem");
    assert!(bad.said.told(&es).ends_with(" debe cumplir [a-z0-9_]{1,32}"), "{}", bad.said.told(&es));
    let shown = serde_json::to_value(bad).unwrap();
    assert_eq!(shown["said"]["id"], json!("scene.id"));
    assert_eq!(
        shown["message"],
        json!(format!("{} must match [a-z0-9_]{{1,32}}", shown["said"]["args"][0].as_str().unwrap()))
    );
}
