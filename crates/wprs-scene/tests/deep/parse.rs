use super::*;

#[test]
fn every_advanced_field_parses_validates_and_names_its_feature() {
    let scene = deep();
    let mut wanted: Vec<&str> = EXPECTED.to_vec();
    wanted.sort();
    assert_eq!(scene.features().into_iter().collect::<Vec<_>>(), wanted);
    let uses = needs(&scene);
    assert!(uses.audio && uses.cursor && uses.clicks && uses.media && uses.scripts);
    let Kind::Mesh(fox) = &scene.layer("fox").unwrap().kind else { panic!("fox is no mesh") };
    assert_eq!((fox.rig.clips.len(), fox.rig.ik[0].bend), (2, Bend::Counterclockwise));
    assert_eq!(fox.rig.clips[1].repeat, Loop::Once);
    assert_eq!(scene.layer("fox").unwrap().solid, Solid::Hit { alpha: 0.5 });
    let Kind::Model(robot) = &scene.layer("robot").unwrap().kind else { panic!("robot is no model") };
    assert_eq!(robot.scale, Some(Value::Fixed(Vec3([1.5; 3]))));
    assert_eq!(robot.materials["hair"].blend, Some(Coverage::Mask));
    let fireworks = &scene.particles["fireworks"];
    assert_eq!(
        (fireworks.space, fireworks.init.life.exponent, fireworks.forces[0].window),
        (Placement::World, Some(2.0), Some([0.0, 0.1, 0.8, 1.0]))
    );
    assert_eq!(fireworks.render.stretch, Stretch { time: 0.1, min: Some(2.0), max: Some(40.0) });
    let EmitShape::Box(area) = fireworks.emitters[0].shape else { panic!("no box") };
    assert_eq!((area.inner, area.at), (Some([50.0, 50.0]), Some(Handle(1))));
    assert_eq!(scene.scripts[0].events.len(), 6);
    assert_eq!(scene.camera3d.as_ref().unwrap().paths[0].keys.len(), 2);
    assert!(scene.layer("lights").is_some_and(|l| matches!(l.kind, Kind::Adjustment { rgb: true, .. })));
}

#[test]
fn advanced_fields_round_trip_and_older_forms_still_read() {
    let scene = deep();
    let text = scene.to_json().unwrap();
    assert_eq!(Scene::from_json(&text).unwrap(), scene);
    let written: Json = serde_json::from_str(&text).unwrap();
    assert_eq!(written["particles"]["fireworks"]["init"]["life"], json!([1.0, 3.0, 2.0]));
    assert_eq!(written["particles"]["fireworks"]["forces"][0]["window"], json!([0.0, 0.1, 0.8, 1.0]));
    assert_eq!(written["layers"][2]["space"], json!("3d"));
    let old = json!({ "capacity": 10, "emitters": [{ "shape": { "box": [[0.0, 0.0], [10.0, 10.0]] }, "rate": 1 }],
        "init": { "life": [1, 2] }, "render": { "stretch": 0.5 }, "forces": [{ "drag": 0.1 }] });
    let system: System = serde_json::from_value(old.clone()).unwrap();
    assert_eq!(serde_json::to_value(&system).unwrap()["emitters"][0]["shape"], old["emitters"][0]["shape"]);
    assert_eq!((system.init.life.ends, system.render.stretch.time), ([1.0, 2.0], 0.5));
    let older = Scene::from_json(include_str!("../sample.json")).unwrap();
    assert!(older.features().is_empty() && !needs(&older).media);
}

#[test]
fn models_are_read_from_their_files_and_checked_by_name() {
    let mut scene = deep();
    adopt(&mut scene, &figure()).unwrap_or_else(|e| panic!("{e}"));
    let model = &scene.models["fox"];
    assert_eq!((model.triangles, model.skinned, model.animations["wave"]), (300, true, 3.5));
    assert!(model.bones.contains("hand") && model.nodes.contains("socket") && model.shapes.contains("blink"));
    assert_eq!(scene.loaded["glass"].stage, Stage::Material);
    for (path, value, fault) in [
        (
            "/layers/0/ik/0/chain/1",
            json!("tail"),
            "/layers/0/ik/0/chain/1 names \"tail\", which its model does not have",
        ),
        ("/layers/0/clips/0/animation", json!("run"), "/layers/0/clips/0/animation names \"run\""),
        ("/layers/0/clips/0/to", json!(5), "/layers/0/clips/0/to is 5, expected"),
        ("/layers/1/attach/bone", json!("wing"), "/layers/1/attach/bone names \"wing\""),
        ("/layers/0/parts/leg", json!({ "order": 1 }), "/layers/0/parts/leg names \"leg\""),
    ] {
        let mut doc: Json = serde_json::from_str(DEEP).unwrap();
        let (parent, key) = path.rsplit_once('/').unwrap();
        match doc.pointer_mut(path) {
            Some(slot) => *slot = value,
            None => doc.pointer_mut(parent).unwrap()[key] = value,
        }
        let mut broken = Scene::from_json(&doc.to_string()).unwrap();
        let found = adopt(&mut broken, &figure()).unwrap_err().to_string();
        assert!(found.contains(fault), "{found}");
    }
    let mut outside = figure();
    outside["buffers"] = json!([{ "uri": "https://example.org/data.bin", "byteLength": 4 }]);
    let mut broken = deep();
    assert!(
        adopt(&mut broken, &outside).unwrap_err().to_string().contains("loads files from outside itself")
    );
    let mut extended = figure();
    extended["extensionsRequired"] = json!(["EXT_secret"]);
    assert!(ModelFile::parse(&glb(&extended)).unwrap_err().contains("EXT_secret"));
    assert!(ModelFile::parse(b"glTF\x01\0\0\0").unwrap_err().contains("binary glTF 2.0"));
    let mut crowd = figure();
    crowd["skins"] = json!([{ "joints": vec![0; 300] }]);
    assert!(ModelFile::parse(&glb(&crowd)).unwrap_err().contains("300 bones"));
    let mut heavy = figure();
    heavy["accessors"][3]["count"] = json!(9_000_000);
    assert!(ModelFile::parse(&glb(&heavy)).unwrap_err().contains("triangles"));
    let mut truncated = glb(&figure());
    truncated.pop();
    assert!(ModelFile::parse(&truncated).unwrap_err().contains("length"));
}

#[test]
fn shader_stages_are_checked_against_their_use() {
    let mut scene = deep();
    let mut doc: Json = serde_json::from_str(DEEP).unwrap();
    doc["layers"][4]["effects"] = json!([{ "effect": "glass" }]);
    let mut misplaced = Scene::from_json(&doc.to_string()).unwrap();
    let found = adopt(&mut misplaced, &figure()).unwrap_err().to_string();
    assert!(found.contains("/layers/4/effects/0/effect does not fit"), "{found}");
    adopt(&mut scene, &figure()).unwrap();
    let wrong = Descriptor::packaged(
        "glass",
        &json!({ "id": "glass", "stage": "material", "cost": {} }).to_string(),
        "fn other() {}",
    );
    assert!(wrong.unwrap_err().contains("fn material(value: Surface) -> Surface"));
    let passes =
        json!({ "id": "glass", "stage": "material", "cost": {}, "passes": [{ "entry": "fs_glass" }] });
    assert!(Descriptor::packaged("glass", &passes.to_string(), GLASS).is_err());
    let sparkle = Descriptor::packaged(
        "sparkle",
        &json!({ "id": "sparkle", "stage": "particle", "cost": {} }).to_string(),
        SPARKLE,
    )
    .unwrap();
    assert!(sparkle.uses.contains(&Use::Time) && sparkle.newer());
}
