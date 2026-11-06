use serde_json::{Value, json};
use wprs_editor::keys::value_at;
use wprs_scene::{Color, Key, Vec2, Vec3, sample};

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-3
}

fn rendered(keys: &Value, t: f32) -> Vec2 {
    let keys: Vec<Key<Vec2>> = serde_json::from_value(keys.clone()).unwrap();
    sample(&keys, t)
}

#[test]
fn number_keys_of_a_pair_bend_the_same_in_the_editor_and_the_render() {
    for keys in [
        json!([[0, 1, {"out": [1, 0.9]}], [2, 3]]),
        json!([[0, 1], [2, 3, {"in": [0.5, -1]}]]),
        json!([[0, [1, 1], {"out": [1, 0.9]}], [2, [3, 3]]]),
        json!([[0, [1, 2], {"out": [1, 0.2]}], [2, [3, -1]]]),
    ] {
        for t in [0.25, 0.5, 1.0, 1.5, 1.9] {
            let edited = value_at(&json!({ "keys": keys }), f64::from(t));
            let edited = match &edited {
                Value::Number(n) => [n.as_f64().unwrap(); 2],
                list => [list[0].as_f64().unwrap(), list[1].as_f64().unwrap()],
            };
            let shown = rendered(&keys, t);
            assert!(
                near(edited[0], f64::from(shown.0[0])) && near(edited[1], f64::from(shown.0[1])),
                "{keys} at {t}: editor {edited:?}, render {:?}",
                shown.0
            );
        }
    }
}

#[test]
fn a_number_key_of_a_pair_is_written_back_as_the_number_it_was() {
    let keys: Vec<Key<Vec2>> =
        serde_json::from_value(json!([[0, 1, {"out": [1, 0.5]}], [2, [3, 3]]])).unwrap();
    assert_eq!(
        serde_json::to_value(&keys).unwrap(),
        json!([[0.0, 1.0, {"out": [1.0, 0.5]}], [2.0, [3.0, 3.0]]])
    );
    let triple: Vec<Key<Vec3>> = serde_json::from_value(json!([[0, 0.5], [1, [1, 2, 3]]])).unwrap();
    assert_eq!((triple[0].one, triple[0].v.0, triple[1].one), (Some(0.5), [0.5; 3], None));
    assert!(serde_json::from_value::<Vec<Key<Color>>>(json!([[0, 0.5]])).is_err(), "a color is no number");
}
