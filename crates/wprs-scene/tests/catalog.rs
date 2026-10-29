use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use wprs_scene::*;

fn scene(more: Json) -> Scene {
    let mut doc = json!({ "schema": SCHEMA, "canvas": { "size": [64, 32] }, "assets": { "sky": { "path": "sky.png" } } });
    doc.as_object_mut().unwrap().extend(more.as_object().unwrap().clone());
    Scene::from_json(&doc.to_string()).unwrap_or_else(|e| panic!("{more}: {e}"))
}

fn issues(more: Json) -> Vec<String> {
    scene(more).validate(&BTreeMap::new()).iter().map(ToString::to_string).collect()
}

fn effect(e: Json) -> Json {
    json!({ "layers": [{ "id": "sky", "type": "image", "image": "sky", "effects": [e] }] })
}

#[test]
fn values_of_described_parameters_are_named_in_english_and_spanish() {
    let newer = builtin().iter().flat_map(|b| newer_params(&b.id));
    let all = newer.chain(undrawn().iter().flat_map(|d| &d.params));
    let labels: Vec<_> = all.flat_map(|p| &p.values).map(|e| &e.label).collect();
    assert!(labels.len() > 20 && labels.iter().all(|l| l.contains_key("en") && l.contains_key("es")));
}

#[test]
fn described_effects_are_complete_and_drawn_only_with_a_shader() {
    assert!(undrawn().len() >= 18);
    for d in undrawn() {
        let problems: Vec<String> =
            d.problems().into_iter().filter(|p| !p.contains("a fused effect needs")).collect();
        assert!(problems.is_empty(), "{problems:?}");
        assert!(d.name.contains_key("en") && reserved(&d.id) && !drawn(&d.id, None), "{}", d.id);
        assert!(descriptor(&d.id).is_none() && described(&d.id) == Some(d), "{}", d.id);
    }
    for b in builtin() {
        let more: Vec<&ParamDef> = newer_params(&b.id).collect();
        let full = described(&b.id).unwrap();
        for p in &more {
            let shown: Vec<&ParamDef> = full.params.iter().filter(|o| o.id == p.id).collect();
            assert!(
                p.accepts(&p.default) && matches!(shown[..], [o] if o.kind == p.kind),
                "{}.{}",
                b.id,
                p.id
            );
            assert_eq!(drawn(&b.id, Some(&p.id)), b.params.iter().any(|o| o.id == p.id), "{}.{}", b.id, p.id);
        }
        assert!(full.params.iter().all(|p| b.params.contains(p) || more.contains(&p)));
        assert!(descriptor(&b.id) == Some(b) && drawn(&b.id, b.params.first().map(|p| p.id.as_str())));
    }
    assert!(
        newer_params("waves").any(|p| p.id == "phase") && newer_params("god_rays").any(|p| p.id == "edges")
    );
}

#[test]
fn described_effects_and_parameters_are_validated_like_built_in_ones() {
    let ok = effect(json!({ "effect": "light_shafts", "params": { "intensity": 2, "gradient": "sky" } }));
    assert_eq!(issues(ok), Vec::<String>::new());
    assert_eq!(
        issues(effect(json!({ "effect": "blur", "params": { "blend": "screen", "amount": 0.5 } }))),
        Vec::<String>::new()
    );
    let cases = [
        (
            json!({ "effect": "light_shafts", "params": { "intensity": 9 } }),
            "/layers/0/effects/0/params/intensity is 9, expected 0..=4",
        ),
        (
            json!({ "effect": "blur", "params": { "blend": "sparkle" } }),
            "/layers/0/effects/0/params/blend does not fit",
        ),
        (
            json!({ "effect": "god_rays", "params": { "edges": 2.5 } }),
            "/layers/0/effects/0/params/edges does not fit",
        ),
        (
            json!({ "effect": "tint", "params": { "mode": 4 } }),
            "/layers/0/effects/0/params/mode does not fit",
        ),
        (json!({ "effect": "reveal", "params": { "texture": "moon" } }), "names the unknown asset \"moon\""),
        (json!({ "effect": "blur", "params": { "phase": "sky" } }), "names the unknown parameter \"phase\""),
    ];
    for (e, wanted) in cases {
        let found = issues(effect(e.clone()));
        assert!(found.iter().any(|f| f.contains(wanted)), "{e}: {found:?}");
    }
    let mut own = scene(
        json!({ "effects": { "tint": { "descriptor": "effects/tint.json", "wgsl": "effects/tint.wgsl" } } }),
    );
    let found =
        own.adopt(&BTreeMap::new(), |_: &str, _| Ok::<_, String>(Vec::new())).unwrap_err().to_string();
    assert!(found.contains("/effects/tint"), "{found}");
    let follow = scene(effect(json!({ "effect": "reveal", "params": { "follow": true } })));
    let beat = scene(effect(json!({ "effect": "pulse", "params": { "audio_band": 3 } })));
    let still = scene(effect(json!({ "effect": "pulse" })));
    assert!(needs(&follow).cursor && needs(&beat).audio && !needs(&still).audio && !needs(&still).cursor);
}

#[test]
fn timelines_count_frames_wrap_and_seeds_start_fresh() {
    let counted = scene(
        json!({ "timelines": [{ "id": "intro", "fps": 24, "frames": 48, "events": [{ "t": 1.5, "name": "drop" }] }] }),
    );
    assert_eq!((counted.timelines[0].length(), counted.validate(&BTreeMap::new())), (2.0, Vec::new()));
    assert!(
        counted.features().contains("timeline_frames") && !counted.to_json().unwrap().contains("duration")
    );
    let wrapped =
        scene(json!({ "timelines": [{ "id": "intro", "duration": 2, "loop": "repeat", "wrap": true }] }));
    assert!(wrapped.validate(&BTreeMap::new()).is_empty() && wrapped.features().contains("timeline_wrap"));
    let cases = [
        (json!([{ "id": "a", "frames": 48 }]), "/timelines/0 needs either a duration or both frames and fps"),
        (
            json!([{ "id": "a", "duration": 2, "fps": 24, "frames": 48 }]),
            "/timelines/0 needs either a duration",
        ),
        (json!([{ "id": "a", "fps": 500, "frames": 48 }]), "/timelines/0/fps is 500, expected 1..=240"),
        (json!([{ "id": "a", "fps": 24, "frames": 0 }]), "/timelines/0/frames is 0"),
        (json!([{ "id": "a" }]), "/timelines/0/duration is 0"),
        (
            json!([{ "id": "a", "duration": 2, "wrap": true }]),
            "/timelines/0/wrap applies to timelines that loop with repeat only",
        ),
        (
            json!([{ "id": "a", "fps": 24, "frames": 24, "events": [{ "t": 2, "name": "late" }] }]),
            "/timelines/0/events/0/t is 2, expected 0..=1",
        ),
    ];
    for (timelines, wanted) in cases {
        let found = issues(json!({ "timelines": timelines }));
        assert!(found.iter().any(|f| f.contains(wanted)), "{timelines}: {found:?}");
    }
    let fresh = scene(json!({ "clock": { "seed": null } }));
    assert_eq!((fresh.clock.seed, fresh.features().contains("fresh_seed")), (None, true));
    let written: Json = serde_json::from_str(&fresh.to_json().unwrap()).unwrap();
    assert!(written["clock"]["seed"].is_null() && written["clock"].get("seed").is_some(), "{written}");
    let fixed = scene(json!({ "clock": { "seed": 7 } }));
    let runtime = Runtime::new(std::sync::Arc::new(fixed.clone()), Values::default());
    assert_eq!((runtime.seed, fixed.features().contains("fresh_seed")), (7, false));
    assert_eq!(scene(json!({})).clock.seed, Some(0));
}

#[test]
fn the_scene_reference_lists_every_described_effect_and_parameter() {
    let doc = include_str!("../../../docs/specs/scene.md");
    let (_, rest) = doc.split_once("### Effects not drawn yet").unwrap();
    let (section, _) = rest.split_once("\n## ").unwrap();
    for d in undrawn() {
        let row = section
            .lines()
            .find(|l| l.starts_with(&format!("| `{}` |", d.id)))
            .unwrap_or_else(|| panic!("{}", d.id));
        assert!(d.params.iter().all(|p| row.contains(&format!("`{}`", p.id))), "{row}");
    }
    for b in builtin() {
        let mut more = newer_params(&b.id).peekable();
        if more.peek().is_none() {
            continue;
        }
        let named = |l: &&str| l.split('|').nth(1).is_some_and(|c| c.contains(&format!("`{}`", b.id)));
        let row = section.lines().find(named).unwrap_or_else(|| panic!("{}", b.id));
        assert!(more.all(|p| row.contains(&format!("`{}`", p.id))), "{row}");
    }
}
