use super::*;

#[test]
fn shapes_points_and_layer_sources_are_set_undone_and_checked() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let before = p.doc.clone();
    edit(&mut p, "point", json!({ "point": 1, "at": [300, 200], "angle": 45 })).unwrap();
    assert_eq!(system(&p)["points"], json!([{ "at": [0, 0] }, { "at": [300.0, 200.0], "angle": 45.0 }]));
    undone(&mut p, &before);
    let around = json!({ "around": { "at": "p1", "count": 12, "radius": 40, "mode": "mirror" } });
    let before = p.doc.clone();
    edit(&mut p, "shape", json!({ "emitter": 0, "shape": around })).unwrap();
    assert_eq!(system(&p)["emitters"][0]["shape"]["around"]["count"], 12);
    undone(&mut p, &before);
    edit(
        &mut p,
        "shape",
        json!({ "emitter": 1, "shape": { "between": { "from": "p0", "to": "p1", "count": 8 } } }),
    )
    .unwrap();
    assert_eq!(system(&p)["emitters"][1]["rate"], 10);
    let before = p.doc.clone();
    edit(
        &mut p,
        "layer",
        json!({ "emitter": 1, "id": "logo", "threshold": 0.4, "color": true, "refresh": "on_change" }),
    )
    .unwrap();
    let source = &system(&p)["emitters"][1]["shape"]["layer"];
    assert_eq!((source["id"].clone(), source["refresh"].clone()), (json!("logo"), json!("on_change")));
    undone(&mut p, &before);
    edit(&mut p, "point", json!({ "point": 2, "follow": true })).unwrap();
    assert_eq!(system(&p)["points"][2], json!({ "follow": "cursor" }));
    edit(&mut p, "point", json!({ "point": 2, "remove": true })).unwrap();
    assert_eq!(system(&p)["points"].as_array().map(Vec::len), Some(2));
    let bad = [
        ("shape", json!({ "emitter": 0, "shape": { "around": { "count": 0, "radius": 4 } } })),
        ("shape", json!({ "emitter": 5, "shape": "edge" })),
        ("shape", json!({ "emitter": 0, "shape": { "spiral": {} } })),
        ("shape", json!({ "emitter": 0, "shape": { "around": { "at": "p6", "count": 3, "radius": 4 } } })),
        ("point", json!({ "point": 9, "at": [0, 0] })),
        ("point", json!({ "point": 0, "remove": true })),
        ("point", json!({ "point": 1, "angle": 400 })),
        ("layer", json!({ "emitter": 0, "id": "flakes" })),
        ("layer", json!({ "emitter": 0, "id": "nothing" })),
        ("layer", json!({ "emitter": 0, "id": "logo", "threshold": 2 })),
    ];
    for (what, args) in bad {
        let before = p.doc.clone();
        assert!(edit(&mut p, what, args.clone()).is_err(), "{what} {args}");
        assert_eq!(p.doc, before, "{what} {args} changed nothing");
    }
    assert!(
        act(&mut p, json!({ "particles": { "cutout": { "system": "rain", "range": [0, 1, 1] } } })).is_err()
    );
}

#[test]
fn remaps_looks_depth_children_and_shaders_are_set_undone_and_checked() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let remap = json!({ "input": "random", "in": [0, 1], "output": "size", "out": [2, 8] });
    let before = p.doc.clone();
    edit(&mut p, "remap", json!({ "remap": remap })).unwrap();
    edit(
        &mut p,
        "remap",
        json!({ "remap": { "input": "daytime", "in": [0, 1], "output": "alpha", "out": [1, 0] } }),
    )
    .unwrap();
    edit(
        &mut p,
        "remap",
        json!({ "index": 0, "remap": { "input": "life", "in": [0, 10], "output": "spin", "out": [0, 90] } }),
    )
    .unwrap();
    assert_eq!(system(&p)["init"]["remap"][0]["output"], "spin");
    edit(&mut p, "remap", json!({ "index": 1, "remove": true })).unwrap();
    assert_eq!(system(&p)["init"]["remap"].as_array().map(Vec::len), Some(1));
    edit(&mut p, "cutout", json!({ "range": [0.2, 0.6, 0.8] })).unwrap();
    edit(&mut p, "orient", json!({ "orient": "fixed", "angle": 30 })).unwrap();
    edit(&mut p, "perspective", json!({ "focal": 600, "depth": [-100, 300], "depth_speed": [0, 20] }))
        .unwrap();
    let s = system(&p);
    assert_eq!(
        (s["render"]["cutout"].clone(), s["render"]["orient"].clone()),
        (json!([0.2, 0.6, 0.8]), json!("fixed"))
    );
    assert_eq!(
        (s["perspective"]["focal"].clone(), s["init"]["depth"].clone()),
        (json!(600.0), json!([-100.0, 300.0]))
    );
    edit(&mut p, "orient", json!({ "layer": "flakes", "orient": "velocity" })).unwrap();
    let own = &p.doc["scene"]["layers"][1]["render"];
    assert_eq!((own["orient"].clone(), own["cutout"].clone()), (json!("velocity"), json!([0.2, 0.6, 0.8])));
    edit(&mut p, "perspective", json!({ "off": true })).unwrap();
    assert!(system(&p).get("perspective").is_none() && system(&p)["init"].get("depth").is_none());
    let child = json!({ "capacity": 10, "emitters": [{ "shape": { "point": [0, 0] } }] });
    act(&mut p, json!({ "set": { "pointer": "/scene/particles/spark", "value": child } })).unwrap();
    act(&mut p, json!({ "set": { "pointer": "/scene/particles/flakes/children", "value": [{ "system": "spark", "on": "follow" }] } })).unwrap();
    edit(&mut p, "inherit", json!({ "child": 0, "inherit": ["color", "size"], "follow": true })).unwrap();
    assert_eq!(system(&p)["children"][0]["inherit"], json!(["color", "size"]));
    assert_eq!(system(&p)["children"][0]["follow_inherit"], json!(true));
    let after = p.doc.clone();
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert!(system(&p)["children"][0].get("inherit").is_none());
    act(&mut p, json!({ "redo": {} })).unwrap();
    assert_eq!(p.doc, after);
    let _ = before;
    let bad = [
        ("remap", json!({ "remap": { "input": "weather", "in": [0, 1], "output": "size", "out": [0, 1] } })),
        ("remap", json!({ "remap": { "input": "random", "in": [1, 1], "output": "size", "out": [0, 1] } })),
        ("remap", json!({ "remap": { "input": "random", "in": [0, 1], "output": "glow", "out": [0, 1] } })),
        ("remap", json!({ "index": 4, "remove": true })),
        ("remap", json!({})),
        ("cutout", json!({ "range": [0.8, 0.2, 1] })),
        ("perspective", json!({ "focal": 0 })),
        ("inherit", json!({ "child": 3, "inherit": ["color"] })),
        ("inherit", json!({ "child": 0, "inherit": ["color", "color"] })),
        ("shader", json!({ "shader": "nothing" })),
        ("orient", json!({ "layer": "logo", "orient": "screen" })),
    ];
    for (what, args) in bad {
        let before = p.doc.clone();
        assert!(edit(&mut p, what, args.clone()).is_err(), "{what} {args}");
        assert_eq!(p.doc, before, "{what} {args}");
    }
    for _ in 0..8 {
        let _ = edit(&mut p, "remap", json!({ "remap": remap }));
    }
    assert!(edit(&mut p, "remap", json!({ "remap": remap })).is_err(), "at most eight remaps");
}
