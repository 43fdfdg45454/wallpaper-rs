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

#[test]
fn masked_effects_that_mix_by_their_mask_fuse_only_when_they_keep_their_place() {
    let m = json!({ "texture": "b" });
    let tone = json!({ "effect": "color_adjust", "params": { "saturation": 0 }, "mask": m });
    let blocks = json!({ "effect": "pixelate", "params": { "size": 8 }, "mask": m });
    let plain = json!({ "effect": "color_adjust", "params": {} });
    assert_eq!(fusion(&uses(json!([tone, plain.clone()]))).first(), Some(&(0..2)));
    assert!(fusion(&uses(json!([blocks, plain]))).is_empty());
    let chain = ["color_adjust", "tint"].map(|id| descriptor(id).unwrap());
    let mix = effect_source(&chain);
    assert!(mix.contains("mix(c0, color_color_adjust(c, q1), mask_at(q1))"), "{mix}");
    assert!(mix.contains("c = color_tint(c, q2);"), "{mix}");
}
