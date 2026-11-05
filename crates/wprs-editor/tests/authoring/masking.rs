use super::{act, at, project, refused};
use serde_json::json;

#[test]
fn an_earlier_broken_field_does_not_hide_a_new_one() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    act(&mut p, json!({"set": {"pointer": "/manifest/authors", "value": [""]}})).unwrap();
    let words = refused(&mut p, json!({"manifest": {"license": {"license": "(("}}}));
    assert!(words.contains("SPDX"), "{words}");
    act(&mut p, json!({"add_property": {"kind": "slider", "id": "speed"}})).unwrap();
    act(&mut p, json!({"add_property": {"kind": "slider", "id": "size"}})).unwrap();
    act(&mut p, json!({"set": {"pointer": "/manifest/properties/speed/default", "value": 900}})).unwrap();
    refused(&mut p, json!({"property": {"id": "size", "default": 900}}));
    let top = at(&p, "/manifest/properties/size/max");
    act(&mut p, json!({"property": {"id": "size", "default": top}})).unwrap();
    assert_eq!(at(&p, "/manifest/properties/size/default"), top);
}

#[test]
fn a_license_and_the_authors_are_bounded() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    refused(&mut p, json!({"manifest": {"license": {"license": "a".repeat(300)}}}));
    let many: Vec<String> = (0..65).map(|i| format!("a{i}")).collect();
    refused(&mut p, json!({"manifest": {"authors": {"authors": many}}}));
    act(&mut p, json!({"manifest": {"authors": {"authors": &many[..64]}}})).unwrap();
}

#[test]
fn ambiguous_or_wrong_values_get_the_words_of_the_editor() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    act(&mut p, json!({"add_layer": {"kind": "solid", "id": "hole"}})).unwrap();
    act(&mut p, json!({"effects": {"add": {"layer": "card", "effect": "vignette"}}})).unwrap();
    let both = json!({"effects": {"mask": {"layer": "card", "index": 0, "from": "hole", "texture": "x"}}});
    assert!(refused(&mut p, both).contains("one of"));
    assert!(refused(&mut p, json!({"canvas": {"bloom": {"hdr": 1}}})).contains("glare"));
    act(&mut p, json!({"add_layer": {"kind": "particles", "id": "dust"}})).unwrap();
    let start = json!({"system": "dust", "ranges": {"color": ["#ffffff", "notacolor"]}});
    assert!(refused(&mut p, json!({"particles": {"start": start}})).contains("notacolor is not a color"));
}
