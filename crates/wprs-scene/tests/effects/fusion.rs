use super::*;

fn uses(list: serde_json::Value) -> Vec<EffectUse> {
    serde_json::from_value(list).unwrap()
}

fn waves(mask: serde_json::Value) -> serde_json::Value {
    json!({ "effect": "waves", "params": { "amplitude": 4 }, "mask": mask })
}

#[test]
fn masked_waves_fuse_while_their_masks_fit_two_pictures() {
    let a = json!({ "layer": "a", "channel": "r" });
    let a_green = json!({ "layer": "a", "channel": "g" });
    let b = json!({ "texture": "b", "channel": "b" });
    let c = json!({ "layer": "c" });
    let two = uses(json!([waves(a.clone()), waves(a_green), waves(b.clone()), waves(a.clone())]));
    assert_eq!(fusion(&two).first(), Some(&(0..4)));
    assert_eq!(fusion(&two).len(), 1);
    let three = uses(json!([waves(a.clone()), waves(b.clone()), waves(c), waves(a.clone())]));
    assert_eq!(fusion(&three), [0..2, 2..4]);
    let shaped = json!({ "shape": { "ellipse": { "size": [10, 10] } } });
    let feathered = json!({ "layer": "a", "feather": 2 });
    let apart = uses(json!([waves(a.clone()), waves(shaped), waves(b), waves(feathered), waves(a)]));
    assert!(fusion(&apart).is_empty(), "{:?}", fusion(&apart));
}
