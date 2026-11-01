use super::*;

#[test]
fn newer_effects_refuse_values_outside_their_types_and_ranges() {
    let cases = [
        (json!({ "effect": "fluid", "params": { "render": 7 } }), "/params/render does not fit"),
        (json!({ "effect": "fluid", "params": { "push": 9 } }), "/params/push is 9, expected 0..=8"),
        (json!({ "effect": "fluid", "params": { "blend": "sparkle" } }), "/params/blend does not fit"),
        (
            json!({ "effect": "trail", "params": { "persistence": 1 } }),
            "/params/persistence is 1, expected 0..=0.99",
        ),
        (
            json!({ "effect": "local_contrast", "params": { "radius": 0 } }),
            "/params/radius is 0, expected 1..=64",
        ),
        (json!({ "effect": "glitter", "params": { "size": 32 } }), "/params/size is 32, expected 0.5..=16"),
        (json!({ "effect": "pulse", "params": { "audio_band": 1.5 } }), "/params/audio_band does not fit"),
        (json!({ "effect": "pulse", "params": { "what": 3 } }), "/params/what does not fit"),
        (json!({ "effect": "reveal", "params": { "shape": 2 } }), "/params/shape does not fit"),
        (json!({ "effect": "displace", "params": { "map": "moon" } }), "names the unknown asset \"moon\""),
        (json!({ "effect": "transform", "params": { "skew": "far" } }), "/params/skew does not fit"),
        (
            json!({ "effect": "electric", "params": { "multiply": -1 } }),
            "/params/multiply is -1, expected 0..=8",
        ),
        (json!({ "effect": "god_rays", "params": { "edges": 17 } }), "/params/edges is 17, expected 2..=16"),
        (json!({ "effect": "chromatic_aberration", "params": { "mode": 4 } }), "/params/mode does not fit"),
        (json!({ "effect": "waves", "params": { "count": 3 } }), "/params/count is 3, expected 1..=2"),
        (
            json!({ "effect": "water_ripple", "params": { "walls": "moon" } }),
            "names the unknown asset \"moon\"",
        ),
        (json!({ "effect": "film_grain", "params": { "blend": "tinted" } }), "/params/blend does not fit"),
    ];
    for (e, wanted) in cases {
        let found = issues(effect(e.clone()));
        assert!(found.iter().any(|f| f.contains(wanted)), "{e}: {found:?}");
    }
    let fine = [
        json!({ "effect": "film_grain", "params": { "blend": "diffuse_light", "mono": true, "bias": -1 } }),
        json!({ "effect": "fluid", "params": { "gradient": "sky", "source_strength": 8, "render": 3 } }),
        json!({ "effect": "shake", "params": { "phase": "sky", "bounds": [0.5, 1] } }),
        json!({ "effect": "blur", "params": { "blend": "tint", "amount": 0.4 } }),
    ];
    for e in fine {
        assert_eq!(issues(effect(e.clone())), Vec::<String>::new(), "{e}");
    }
    let busy = scene(effect(json!({ "effect": "fluid", "params": { "source_strength": 4 } })));
    let pushed = scene(effect(json!({ "effect": "fluid" })));
    let quiet = scene(effect(json!({ "effect": "fluid", "params": { "push": 0 } })));
    assert!(needs(&busy).cursor && needs(&pushed).cursor && !needs(&quiet).cursor);
    let frozen = descriptor("fluid").unwrap();
    assert!(frozen.frozen(Quality::Q0) && !frozen.frozen(Quality::Q1) && frozen.class() == Class::Heavy);
    let trail = descriptor("trail").unwrap();
    let mut slots = [[0.0; 4]; MAX_PARAMS];
    slots[0][0] = 0.9;
    assert!((trail.settle(&slots) - 0.987).abs() < 0.01, "{}", trail.settle(&slots));
}
