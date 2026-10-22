use super::{literals, qml};
use crate::editing::describe;
use serde_json::{Value, json};
use wprs_core::edit::{ACTIONS, Edit, Source};

#[test]
fn every_editor_action_is_a_command_line_action() {
    let used: Vec<String> = qml()
        .iter()
        .flat_map(|f| literals(f, ".run(").into_iter().map(String::from).collect::<Vec<_>>())
        .collect();
    for action in ACTIONS {
        assert!(used.iter().any(|u| u == action), "the editor never runs {action}");
    }
    for action in &used {
        assert!(ACTIONS.contains(&action.as_str()), "{action} is not a wprs-ctl edit command");
    }
}

#[test]
fn the_editor_describes_what_its_controls_show() {
    let mut edit = Edit { source: Source { pixels: [4000, 2000], ..Source::default() }, ..Edit::default() };
    let values = |edit: &Edit| serde_json::from_str::<Value>(&describe(edit)).unwrap();
    let blank = values(&edit);
    assert_eq!((&blank["levels"]["temperature"], &blank["levels"]["ev"]), (&json!(6504.0), &json!(0.0)));
    assert_eq!((&blank["frame"], &blank["zoom"]), (&json!([4000, 2000]), &json!(1.0)));
    assert_eq!(blank["tools"]["vi"]["midpoint"], json!(0.5));
    assert_eq!(
        (&blank["tools"]["ov"]["opacity"], &blank["curves"]["g"]["samples"][32]),
        (&json!(1.0), &json!(0.5))
    );
    edit.set("geometry.rotate", "90").unwrap();
    edit.set("ev", "1").unwrap();
    edit.act(
        serde_json::from_value(
            json!({"mask": {"name": "sky", "spec": "radial:0.5,0.5,0.25,0.25", "invert": true}}),
        )
        .unwrap(),
    )
    .unwrap();
    let turned = values(&edit);
    assert_eq!((&turned["frame"], &turned["levels"]["ev"]), (&json!([2000, 4000]), &json!(1.0)));
    assert_eq!(
        turned["masks"]["sky"],
        json!({"kind": "radial", "values": [0.5, 0.5, 0.25, 0.25, 0.5], "invert": true})
    );
    assert!(edit.ops.len() == 1 && turned["tools"]["bl"]["radius"] == json!(0.0));
}
