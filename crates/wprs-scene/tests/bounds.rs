use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use wprs_scene::Scene;

const DEEP: &str = include_str!("deep.json");

fn faults(changes: &[(&str, Json)]) -> Vec<String> {
    let mut doc: Json = serde_json::from_str(DEEP).unwrap();
    for (path, value) in changes {
        let (parent, key) = path.rsplit_once('/').unwrap();
        match doc.pointer_mut(path) {
            Some(slot) => *slot = value.clone(),
            None => doc.pointer_mut(parent).unwrap_or_else(|| panic!("{path}"))[key] = value.clone(),
        }
    }
    match Scene::from_json(&doc.to_string()) {
        Ok(scene) => scene.validate(&BTreeMap::new()).iter().map(ToString::to_string).collect(),
        Err(e) => vec![e.to_string()],
    }
}

fn expect(changes: &[(&str, Json)], fault: &str) {
    let found = faults(changes);
    assert!(found.iter().any(|f| f.contains(fault)), "{changes:?}: expected {fault:?}, found {found:?}");
}

#[test]
fn the_fixture_is_valid() {
    assert_eq!(faults(&[]), Vec::<String>::new());
}

#[test]
fn meshes_models_and_cameras_name_their_broken_fields() {
    let m = "/layers/2/layers/0";
    for (changes, fault) in [
        (vec![("/layers/0/mesh", json!("skin"))], "/layers/0/mesh does not fit"),
        (vec![("/layers/0/mesh", json!("ghost"))], "/layers/0/mesh names the unknown asset \"ghost\""),
        (vec![("/layers/0/clips/1/id", json!("idle"))], "/layers/0/clips/1/id repeats an id"),
        (vec![("/layers/0/animations/0/clip", json!("run"))], "names the unknown clip \"run\""),
        (
            vec![("/layers/0/animations/0/weight", json!(2))],
            "/layers/0/animations/0/weight is 2, expected 0..=1",
        ),
        (vec![("/layers/0/ik/0/chain", json!(["arm"]))], "/layers/0/ik/0/chain is 1, expected 2..=8"),
        (vec![("/layers/0/ik/0/chain", json!(["arm", "arm"]))], "/layers/0/ik/0/chain repeats an id"),
        (
            vec![("/layers/0/ik/0/target", json!("layer:fox"))],
            "/layers/0/ik/0/target names the unknown layer",
        ),
        (vec![("/layers/0/ik/0/target", json!("nose"))], "names the unknown target \"nose\""),
        (vec![("/layers/0/ik/0/limits", json!({ "leg": [0, 1] }))], "unknown bone of the chain \"leg\""),
        (vec![("/layers/0/ik/0/limits/arm", json!([90, -90]))], "/layers/0/ik/0/limits/arm/1 is -90"),
        (
            vec![("/layers/0/physics/0/stiffness", json!(2))],
            "/layers/0/physics/0/stiffness is 2, expected 0..=1",
        ),
        (vec![("/layers/0/physics/0/mode", json!("jelly"))], "unknown variant `jelly`"),
        (vec![("/layers/0/rules/0/parent", json!("sword"))], "/layers/0/rules/0/parent depends on itself"),
        (vec![("/layers/0/depth/scale", json!(20))], "/layers/0/depth/scale is 20, expected 0..=10"),
        (vec![("/layers/0/expressions/smile/cheeks", json!(3))], "/layers/0/expressions/smile/cheeks is 3"),
        (vec![("/layers/0/shapes/smile", json!(2))], "/layers/0/shapes/smile is 2, expected 0..=1"),
        (
            vec![("/layers/0/parts/arm/clip_to", json!("arm"))],
            "/layers/0/parts/arm/clip_to names the unknown part",
        ),
        (vec![("/layers/1/attach/layer", json!("badge"))], "names the unknown mesh or model layer \"badge\""),
        (vec![("/layers/1/attach/point", json!("socket"))], "/layers/1/attach does not fit"),
        (vec![("/layers/1/parent", json!("fox"))], "/layers/1/attach does not fit"),
        (
            vec![("/layers/3/depth_test", json!(true))],
            "/layers/3/depth_test applies inside a group with space 3d",
        ),
        (vec![(&format!("{m}/materials/metal/params/metallic"), json!(3))], "metallic is 3, expected 0..=1"),
        (
            vec![(&format!("{m}/materials/metal/params/shine"), json!(1))],
            "names the unknown parameter \"shine\"",
        ),
        (vec![(&format!("{m}/materials/metal/shader"), json!("lava"))], "names the unknown shader \"lava\""),
        (
            vec![(&format!("{m}/clips"), json!([])), (&format!("{m}/animations"), json!([]))],
            "root_motion does not fit",
        ),
        (vec![("/layers/2/layers/1/light", json!("tube"))], "/layers/2/layers/1/shadows does not fit"),
        (vec![("/camera3d/fov", json!(200))], "/camera3d/fov is 200, expected 1..=179"),
        (vec![("/camera3d/far", json!(0.01))], "/camera3d/far is 0.01"),
        (vec![("/camera3d/up", json!([0, 0, 0]))], "/camera3d/up does not fit"),
        (
            vec![("/camera3d/paths/0/keys", json!([{ "t": 0, "eye": 1, "center": 0 }]))],
            "/camera3d/paths/0/keys is 1, expected 2..=256",
        ),
        (vec![("/camera3d/paths/0/keys/1/fov", json!(0))], "/camera3d/paths/0/keys/1/fov is 0"),
    ] {
        expect(&changes, fault);
    }
}

#[test]
fn particle_systems_name_their_broken_fields() {
    let f = "/particles/fireworks";
    let point = json!({ "at": [0, 0] });
    for (changes, fault) in [
        (
            vec![(format!("{f}/emitters/0/shape/box/at"), json!("p7"))],
            "names the unknown control point \"p7\"".to_string(),
        ),
        (
            vec![(format!("{f}/emitters/1/shape/circle/at"), json!("p9"))],
            "expected p0 to p7, found \"p9\"".into(),
        ),
        (
            vec![(format!("{f}/emitters/0/shape/box/inner"), json!([500, 0]))],
            format!("{f}/emitters/0/shape/box/inner/0 is 500"),
        ),
        (
            vec![(format!("{f}/emitters/0/periodic/on"), json!([0, 1]))],
            format!("{f}/emitters/0/periodic/on/0 is 0"),
        ),
        (
            vec![(format!("{f}/emitters/0/max_per_step"), json!(0))],
            format!("{f}/emitters/0/max_per_step is 0, expected 1..=1000"),
        ),
        (
            vec![(format!("{f}/emitters/1/shape/circle/sign"), json!([2, 0]))],
            format!("{f}/emitters/1/shape/circle/sign/0 is 2"),
        ),
        (
            vec![(format!("{f}/emitters/2/shape/layer/id"), json!("ghost"))],
            "names the unknown layer \"ghost\"".into(),
        ),
        (
            vec![(format!("{f}/emitters/3/shape/around/count"), json!(0))],
            format!("{f}/emitters/3/shape/around/count is 0"),
        ),
        (
            vec![(format!("{f}/init/alpha"), json!([0.5, 2]))],
            format!("{f}/init/alpha/1 is 2, expected 0.5..=1"),
        ),
        (
            vec![(format!("{f}/init/life"), json!([1, 3, 20]))],
            format!("{f}/init/life/2 is 20, expected 0.1..=10"),
        ),
        (
            vec![(format!("{f}/init/hsv"), json!({ "hue": [0, 90] }))],
            format!("{f}/init/palette does not fit"),
        ),
        (
            vec![(format!("{f}/init/palette/colors"), json!([]))],
            format!("{f}/init/palette/colors is 0, expected 1..=10"),
        ),
        (
            vec![(format!("{f}/init/inherit/from"), json!([1, 2]))],
            format!("{f}/init/inherit/from does not fit"),
        ),
        (
            vec![(format!("{f}/init/remap/0/input"), json!("weather"))],
            "names the unknown remap input \"weather\"".into(),
        ),
        (vec![(format!("{f}/init/remap/0/in"), json!([1, 1]))], format!("{f}/init/remap/0/in does not fit")),
        (vec![(format!("{f}/perspective"), Json::Null)], format!("{f}/init/depth does not fit")),
        (
            vec![(format!("{f}/forces/0/window"), json!([0.5, 0.1, 0.8, 1]))],
            format!("{f}/forces/0/window does not fit"),
        ),
        (
            vec![(format!("{f}/forces/2/vortex/outer"), json!(5))],
            format!("{f}/forces/2/outer is 5, expected 10..=100000"),
        ),
        (
            vec![(format!("{f}/forces/4/oscillate/freq"), json!([2, 1]))],
            format!("{f}/forces/4/oscillate/freq/1 is 1"),
        ),
        (vec![(format!("{f}/forces/11/boids/radius"), json!(0))], format!("{f}/forces/11/boids/radius is 0")),
        (vec![(format!("{f}/capacity"), json!(10_000))], format!("{f}/capacity is 10000, expected 1..=8192")),
        (
            vec![(format!("{f}/children/0/system"), json!("fireworks"))],
            format!("{f}/children depends on itself"),
        ),
        (
            vec![(format!("{f}/children/0/system"), json!("ghost"))],
            "names the unknown particle system \"ghost\"".into(),
        ),
        (
            vec![(format!("{f}/children/0/follow_inherit"), json!(true))],
            format!("{f}/children/0/inherit does not fit"),
        ),
        (
            vec![
                ("/particles/smoke/children".into(), json!([{ "system": "spare", "on": "spawn" }])),
                ("/particles/spare".into(), json!({ "capacity": 10, "emitters": [{ "shape": "edge" }] })),
            ],
            format!("{f}/children nests particle systems more than two levels deep"),
        ),
        (vec![(format!("{f}/render/frames"), json!("time"))], format!("{f}/render/cycles does not fit")),
        (vec![(format!("{f}/render/orient"), json!("velocity"))], format!("{f}/render/angle does not fit")),
        (vec![(format!("{f}/render/cutout"), json!([0.8, 0.2, 1]))], format!("{f}/render/cutout/1 is 0.2")),
        (
            vec![(format!("{f}/render/trail/length"), json!(100))],
            format!("{f}/render/trail/length is 100, expected 1..=64"),
        ),
        (
            vec![(format!("{f}/render/stretch"), json!({ "min": 50, "max": 10 }))],
            format!("{f}/render/stretch/max is 10"),
        ),
        (vec![(format!("{f}/render/rope/subdivide"), json!(9))], format!("{f}/render/rope/subdivide is 9")),
        (
            vec![(format!("{f}/collide/0/bounds/box/to"), json!([0, 0]))],
            format!("{f}/collide/0/bounds does not fit"),
        ),
        (
            vec![(format!("{f}/points"), json!(vec![point.clone(); 9]))],
            format!("{f}/points has 9 entries, at most 8"),
        ),
        (vec![(format!("{f}/points/1/at"), json!([0, 0]))], format!("{f}/points/1 needs exactly one source")),
        (vec![(format!("{f}/lock"), json!(["size", "size"]))], format!("{f}/lock does not fit")),
        (
            vec![("/layers/3/instance/count".into(), json!(0.5))],
            "/layers/3/instance/count is locked by its particle system".into(),
        ),
        (
            vec![("/layers/3/instance/brightness".into(), json!(20))],
            "/layers/3/instance/brightness is 20, expected 0..=16".into(),
        ),
        (
            vec![(format!("{f}/emitters"), json!(vec![json!({ "shape": "edge" }); 9]))],
            format!("{f}/emitters has 9 entries, at most 8"),
        ),
    ] {
        let changes: Vec<(&str, Json)> = changes.iter().map(|(p, v)| (p.as_str(), v.clone())).collect();
        expect(&changes, &fault);
    }
}

#[test]
fn scripts_media_and_pointer_hooks_name_their_broken_fields() {
    let script = |n: usize| json!({ "path": format!("scripts/{n}.js") });
    for (changes, fault) in [
        (vec![("/scripts/0/events", json!(["click", "click"]))], "/scripts/0/events repeats an id"),
        (vec![("/scripts/0/events", json!(["shake"]))], "unknown variant `shake`"),
        (vec![("/scripts/0/uses", json!(["network"]))], "unknown variant `network`"),
        (
            vec![("/scripts/0/params/speed/default", json!(20))],
            "/scripts/0/params/speed/default is 20, expected 0..=10",
        ),
        (vec![("/scripts/0/params/tint/default", json!(1))], "/scripts/0/params/tint/default does not fit"),
        (vec![("/scripts", (0..65).map(script).collect())], "/scripts has 65 entries, at most 64"),
        (vec![("/pools", json!({}))], "/scripts/0/uses does not fit"),
        (
            vec![("/layers/0/solid", json!(false)), ("/layers/0/hit", json!([])), ("/timelines", json!([]))],
            "/scripts/0/events does not fit",
        ),
        (
            vec![("/layers/4/opacity/params", json!({ "speed": "fast" }))],
            "/layers/4/opacity/params/speed does not fit",
        ),
        (
            vec![("/layers/4/opacity/params", json!({ "pace": 1 }))],
            "names the unknown script parameter \"pace\"",
        ),
        (
            vec![("/layers/4/opacity/params/speed", json!(11))],
            "/layers/4/opacity/params/speed is 11, expected 0..=10",
        ),
        (vec![("/layers/4/opacity/fn", json!("1go"))], "/layers/4/opacity/fn does not fit"),
        (
            vec![("/layers/4/opacity", json!({ "keys": [[0, 1]], "fn": "x" }))],
            "/layers/4/opacity does not fit",
        ),
        (
            vec![("/layers/2/layers/2/color", json!({ "media": "title" }))],
            "/layers/2/layers/2/color/media does not fit",
        ),
        (
            vec![("/layers/2/layers/2/text", json!({ "media": "progress" }))],
            "/layers/2/layers/2/text/media does not fit",
        ),
        (vec![("/layers/2/layers/2/text", json!({ "media": "lyrics" }))], "did not match any variant"),
        (vec![("/timelines/2/trigger", json!("media:skip"))], "names the unknown media trigger \"skip\""),
        (
            vec![("/timelines/0/trigger", json!("click:show"))],
            "names the unknown layer that takes clicks \"show\"",
        ),
        (
            vec![("/timelines/1/trigger", json!("leave:badge"))],
            "names the unknown layer that takes the pointer \"badge\"",
        ),
        (vec![("/timelines/3/trigger", json!("event:jump"))], "names the unknown event \"jump\""),
        (
            vec![("/templates/fox", json!({ "id": "fox", "type": "solid", "color": "#000000" }))],
            "/templates/fox repeats an id",
        ),
        (vec![("/templates/bar/opacity", json!(2))], "/templates/bar/opacity is 2, expected 0..=1"),
        (vec![("/pools/waves/max", json!(1000))], "/pools/waves/max is 1000, expected 1..=256"),
        (vec![("/pools/waves/max_vertices", json!(2))], "/pools/waves/max_vertices is 2, expected 3..=65536"),
        (vec![("/layers/0/solid", json!({ "alpha": 2 }))], "/layers/0/solid/alpha is 2, expected 0..=1"),
        (
            vec![(
                "/layers/4/effects",
                json!([{ "effect": "blur", "name": "soft" }, { "effect": "blur", "name": "soft" }]),
            )],
            "/layers/4/effects/1/name repeats an id",
        ),
        (vec![("/layers/4/effects/0/name", json!("Soft Glow"))], "/layers/4/effects/0/name must match"),
        (vec![("/assets/cover/path", json!("song.mp3"))], "/assets/cover/media does not fit"),
    ] {
        expect(&changes, fault);
    }
}
