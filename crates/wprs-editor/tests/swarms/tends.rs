use super::*;
use wprs_scene::Quality;

fn refused(p: &mut Project, cases: &[(&str, Value)]) {
    for (what, args) in cases {
        let before = p.doc.clone();
        assert!(edit(p, what, args.clone()).is_err(), "{what} {args}");
        assert_eq!(p.doc, before, "{what} {args} changed nothing");
    }
}

#[test]
fn colliders_and_their_responses_are_set_undone_and_checked() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let before = p.doc.clone();
    let inside = json!({ "bounds": { "box": { "from": [100, 100], "to": [500, 400], "inside": true } }, "bounce": 0.5 });
    edit(&mut p, "collider", json!({ "collider": inside })).unwrap();
    assert_eq!(system(&p)["collide"], json!([inside]));
    undone(&mut p, &before);
    let c = |p: &Project| system(p)["collide"][0].clone();
    for (response, more, want) in [
        ("stick", json!({}), json!({ "bounce": 0.0, "stick": true })),
        ("stop", json!({}), json!({ "bounce": 0.0, "friction": 1.0 })),
        ("bounce", json!({ "bounce": 0.8, "friction": 0.2 }), json!({ "bounce": 0.8, "friction": 0.2 })),
        ("kill", json!({}), json!({ "kill": true })),
    ] {
        let mut args = json!({ "index": 0, "response": response });
        more.as_object().unwrap().iter().for_each(|(k, v)| args[k] = v.clone());
        edit(&mut p, "response", args).unwrap();
        want.as_object().unwrap().iter().for_each(|(k, v)| assert_eq!(c(&p)[k], *v, "{response} {k}"));
        assert_eq!(handles(&p)["colliders"][0]["response"], response);
    }
    edit(
        &mut p,
        "collider",
        json!({ "collider": { "bounds": { "segment": { "from": [0, 900], "to": [1920, 900] } } } }),
    )
    .unwrap();
    edit(&mut p, "collider", json!({ "collider": { "bounds": { "layer": { "id": "logo" } } } })).unwrap();
    let shown = handles(&p)["colliders"].clone();
    assert_eq!(shown[0]["corners"].as_array().map(Vec::len), Some(4));
    assert_eq!(
        (shown[1]["from"].clone(), shown[1]["to"].clone()),
        (json!([0.0, 900.0]), json!([1920.0, 900.0]))
    );
    let field = shown[2]["field"].as_array().unwrap();
    assert!(field.len() > 20 && shown[2]["approximate"] == false, "{}", shown[2]);
    let inner = field.iter().filter(|f| f[2].as_f64().unwrap() < 0.0).count();
    assert!(inner > 0 && inner < field.len(), "the field has an inside and an outside");
    edit(&mut p, "collider", json!({ "index": 1, "remove": true })).unwrap();
    assert_eq!(system(&p)["collide"].as_array().map(Vec::len), Some(2));
    refused(
        &mut p,
        &[
            ("collider", json!({ "collider": { "bounds": { "layer": { "id": "flakes" } } } })),
            ("collider", json!({ "index": 7, "collider": { "bounds": "canvas" } })),
            ("collider", json!({ "collider": { "bounds": "wall" } })),
            ("collider", json!({ "collider": { "bounds": { "box": { "from": [1, 1], "to": [1, 1] } } } })),
            ("collider", json!({ "collider": { "bounds": "canvas", "bounce": 2 } })),
            ("response", json!({ "index": 9, "response": "stick" })),
            ("response", json!({ "index": 0, "response": "bounce", "bounce": 3 })),
            ("collider", json!({ "index": 9, "remove": true })),
        ],
    );
}

#[test]
fn keeps_flocks_and_operators_are_set_undone_and_checked() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let before = p.doc.clone();
    let ring = json!({ "keep_distance": { "target": "p0", "distance": 50, "strength": 3 } });
    edit(&mut p, "force", json!({ "force": ring })).unwrap();
    assert_eq!(system(&p)["forces"][2], ring);
    undone(&mut p, &before);
    edit(
        &mut p,
        "force",
        json!({ "force": { "keep_between": { "from": [0, 0], "to": [100, 100], "strength": 2 } } }),
    )
    .unwrap();
    let keeps = handles(&p)["keeps"].clone();
    assert_eq!((keeps[0]["radius"].as_f64(), keeps[0]["ring"].clone()), (Some(50.0), json!([50.0, 0.0])));
    assert_eq!(
        (keeps[1]["from"].clone(), keeps[1]["to"].clone()),
        (json!([0.0, 0.0]), json!([100.0, 100.0])),
        "{keeps}"
    );
    let before = p.doc.clone();
    edit(&mut p, "boids", json!({})).unwrap();
    edit(&mut p, "boids", json!({ "boids": { "radius": 60 } })).unwrap();
    let flock = handles(&p)["boids"].clone();
    assert_eq!(
        (flock["index"].as_u64(), flock["radius"].as_f64(), flock["boids"]["cohesion"].as_f64()),
        (Some(4), Some(60.0), Some(0.5))
    );
    edit(&mut p, "boids", json!({ "off": true })).unwrap();
    assert_eq!(p.doc, before);
    let remap = json!({ "input": "age", "in": [0, 4], "output": "alpha", "out": [1, 0] });
    edit(&mut p, "force", json!({ "force": { "remap": remap } })).unwrap();
    let operators = handles(&p)["operators"].clone();
    assert_eq!(operators[0]["index"], 4);
    let points = operators[0]["curve"]["points"].as_array().unwrap();
    assert!((points[points.len() / 2][1].as_f64().unwrap() - 0.5).abs() < 0.05);
    assert!(handles(&p)["steady_inputs"].as_array().unwrap().contains(&json!("age")));
    assert!(!handles(&p)["inputs"].as_array().unwrap().contains(&json!("age")));
    refused(
        &mut p,
        &[
            (
                "force",
                json!({ "force": { "keep_distance": { "target": "p7", "distance": 9, "strength": 1 } } }),
            ),
            ("force", json!({ "force": { "spin": 3 } })),
            ("force", json!({ "index": 12, "remove": true })),
            ("boids", json!({ "boids": { "radius": 0 } })),
            ("boids", json!({ "boids": { "alignment": "much" } })),
        ],
    );
}

#[test]
fn ropes_trails_textures_and_the_budget_follow_the_system() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let before = p.doc.clone();
    edit(&mut p, "strand", json!({ "strand": "rope", "settings": { "subdivide": 3, "width": 6 } })).unwrap();
    edit(&mut p, "strand", json!({ "strand": "trail", "settings": { "length": 48, "segments": 8 } }))
        .unwrap();
    edit(&mut p, "texture", json!({ "texture": "pic" })).unwrap();
    let render = system(&p)["render"].clone();
    assert_eq!(render["rope"], json!({ "subdivide": 3, "width": 6 }));
    assert_eq!(render["trail"], json!({ "length": 48, "width": 0.5, "segments": 8 }));
    let strands = handles(&p)["strands"].clone();
    assert_eq!((strands["texture"].clone(), strands["textures"].clone()), (json!("pic"), json!(["pic"])));
    edit(&mut p, "strand", json!({ "strand": "rope", "layer": "flakes", "settings": { "smooth": false } }))
        .unwrap();
    let own = crate_layer(&p);
    assert_eq!(own["render"]["rope"], json!({ "subdivide": 3, "width": 6, "smooth": false }));
    let budget = wprs_editor::swarms::budget(&p.dir, &p.doc, "flakes", None, 2.0, Quality::Q3);
    let low = wprs_editor::swarms::budget(&p.dir, &p.doc, "flakes", None, 2.0, Quality::Q0);
    let alive = |b: &Value| b["alive"].as_u64().unwrap();
    assert!(alive(&budget) > 100 && alive(&low) * 2 < alive(&budget), "{} {}", alive(&budget), alive(&low));
    assert_eq!(
        (budget["tiers"].as_array().map(Vec::len), budget["levels"][0]["neighbours"].as_u64()),
        (Some(4), Some(8))
    );
    assert_eq!(budget["tiers"][0]["trail"], 0);
    assert!(
        budget["rope"].as_array().unwrap().len() > 10 && !budget["particles"].as_array().unwrap().is_empty()
    );
    assert!(budget["trails"].as_array().unwrap().iter().any(|t| t.as_array().unwrap().len() > 2));
    edit(&mut p, "texture", json!({})).unwrap();
    edit(&mut p, "strand", json!({ "strand": "trail", "off": true })).unwrap();
    assert!(system(&p)["render"].get("trail").is_none() && system(&p)["render"].get("texture").is_none());
    refused(
        &mut p,
        &[
            ("strand", json!({ "strand": "rope", "settings": { "subdivide": 9 } })),
            ("strand", json!({ "strand": "trail", "settings": { "length": 80 } })),
            ("strand", json!({ "strand": "rope", "layer": "logo", "settings": {} })),
            ("texture", json!({ "texture": "nothing" })),
        ],
    );
    let staged = wprs_editor::rigs::staged(&json!({ "layer": "x", "quality": "q1" }));
    assert_eq!(staged, Some(json!({ "quality": "q1", "restart": null })));
    assert_eq!(wprs_editor::rigs::staged(&json!({ "layer": "x" })), None);
    let _ = before;
}

fn crate_layer(p: &Project) -> Value {
    let layers = p.doc["scene"]["layers"].as_array().unwrap();
    layers.iter().find(|l| l["id"] == "flakes").cloned().unwrap()
}
