use super::kit::{act, parts, rigged};
use serde_json::json;
use wprs_editor::Field;
use wprs_editor::inspect::{fields_in, fields_own};

fn find<'a>(list: &'a [Field], pointer: &str) -> &'a Field {
    list.iter().find(|f| f.pointer == pointer).unwrap_or_else(|| panic!("{pointer}"))
}

#[test]
fn fields_that_name_bones_points_and_parts_offer_those_of_their_mesh() {
    let root = tempfile::tempdir().unwrap();
    let mut p = rigged(&root);
    act(
        &mut p,
        json!({ "ik": { "layer": "net", "root": "root", "tip": "arm", "limits": { "arm": [-10, 10] } } }),
    )
    .unwrap();
    act(&mut p, json!({ "physics": { "layer": "net", "bones": ["arm"] } })).unwrap();
    act(
        &mut p,
        json!({ "add": { "pointer": "/scene/layers/-", "value": { "id": "hat", "type": "solid" } } }),
    )
    .unwrap();
    act(&mut p, json!({ "attach": { "layer": "hat", "to": "net", "point": [0.2, 0.5] } })).unwrap();
    let own = Default::default();
    let chain = fields_in(Some(&p.dir), &p.doc, "/scene/layers/0/ik/0", &own);
    let bones = vec![json!("root"), json!("arm")];
    let picked = find(&chain, "/scene/layers/0/ik/0/chain");
    assert_eq!((picked.widget.as_str(), &picked.options), ("refs:bone", &bones));
    let limits = find(&chain, "/scene/layers/0/ik/0/limits");
    assert_eq!((limits.widget.as_str(), &limits.sample), ("map:bone", &json!([0, 0])));
    assert_eq!(find(&chain, "/scene/layers/0/ik/0/limits/arm").widget, "vec2");
    let target = find(&chain, "/scene/layers/0/ik/0/target");
    assert_eq!(target.widget, "ref:reach");
    for option in [json!("cursor"), json!("bone:arm"), json!("bone:hat_point"), json!("layer:hat")] {
        assert!(target.options.contains(&option), "{option}");
    }
    let body = fields_in(Some(&p.dir), &p.doc, "/scene/layers/0/physics/0", &own);
    assert_eq!((find(&body, "/scene/layers/0/physics/0/bone").widget.as_str()), "ref:bone");
    let pin = fields_in(Some(&p.dir), &p.doc, "/scene/layers/1/attach", &own);
    assert_eq!(find(&pin, "/scene/layers/1/attach/bone").options, bones);
    assert!(find(&pin, "/scene/layers/1/attach/point").options.contains(&json!("hat_point")));
    assert_eq!(
        find(&fields_own(&p.doc, "/scene/layers/0/ik/0", &own), "/scene/layers/0/ik/0/chain").widget,
        "lines"
    );
    let root = tempfile::tempdir().unwrap();
    let mut p = parts(&root);
    act(&mut p, json!({ "parts": { "layer": "body", "clip": { "chest": "legs" } } })).unwrap();
    let layer = fields_in(Some(&p.dir), &p.doc, "/scene/layers/0", &own);
    let listed = find(&layer, "/scene/layers/0/parts");
    assert_eq!((listed.widget.as_str(), &listed.options), ("map:part", &vec![json!("legs"), json!("chest")]));
    assert_eq!(find(&layer, "/scene/layers/0/parts/chest/clip_to").options, [json!("legs")]);
}
