use super::*;

fn rigged(extra: Value, layers: Value) -> Scene {
    let mut doc = json!({ "schema": SCHEMA, "canvas": { "size": [200, 100] },
        "assets": { "toy": { "path": "toy.glb" } }, "layers": layers });
    extra.as_object().unwrap().iter().for_each(|(k, v)| doc[k] = v.clone());
    let mut scene = Scene::from_json(&doc.to_string()).unwrap();
    let glb = ForgeRig::new(2.0, 0.5).glb();
    scene.adopt(&BTreeMap::new(), move |_: &str, _: usize| Ok::<_, String>(glb.clone())).unwrap();
    scene
}

fn tip(rt: &Runtime) -> [f32; 12] {
    rt.puppet("m").unwrap().posed().palette[1]
}

const VIEW: [f32; 4] = [0.0, 0.0, 200.0, 100.0];

#[test]
fn an_animated_model_reads_its_skeleton_clips_and_blend_shapes() {
    let s =
        rigged(json!({}), json!([model("m", json!({ "clips": [{ "id": "bend", "animation": "bend" }] }))]));
    let m = &s.solids["toy"];
    let rig = m.rig.as_ref().expect("a posed model keeps its skeleton");
    assert_eq!(rig.joints.iter().map(|j| j.name.as_str()).collect::<Vec<_>>(), ["root", "tip", "body"]);
    assert_eq!((rig.palette.len(), rig.shapes.clone(), rig.motions.len()), (2, vec!["bulge".to_owned()], 2));
    assert_eq!((m.skin.len() / SKIN_FLOATS, m.morphs.len() / 2), (m.vertex_count(), m.vertex_count()));
    assert!((m.bounds[1][1] - 2.0).abs() < 1e-4 && m.painted);
    let still = rigged(json!({}), json!([model("m", json!({}))]));
    assert!(still.solids["toy"].rig.is_none(), "a still model is baked as before");
}

#[test]
fn layers_of_clips_pose_the_model_and_only_while_it_is_shown() {
    let play =
        json!({ "clips": [{ "id": "bend", "animation": "bend" }], "animations": [{ "clip": "bend" }] });
    let mut rt = runtime(rigged(json!({}), json!([model("m", play.clone())])));
    let rest = tip(&rt);
    rt.frame(VIEW, Quality::Q3);
    rt.advance(1.0);
    let bent = tip(&rt);
    assert!((bent[0] - rest[0]).abs() > 0.2, "{rest:?} {bent:?}");
    let mut hidden = play;
    hidden["visible"] = json!(false);
    let mut rt = runtime(rigged(json!({}), json!([model("m", hidden)])));
    rt.frame(VIEW, Quality::Q3);
    let generation = rt.puppet("m").unwrap().posed().generation;
    rt.advance(1.0);
    rt.frame(VIEW, Quality::Q3);
    assert_eq!(rt.puppet("m").unwrap().posed().generation, generation, "a hidden model is not posed");
}

#[test]
fn bone_physics_of_a_model_swings_settles_and_freezes_at_the_lowest_quality() {
    let swing = json!({ "physics": [{ "bone": "tip", "mode": "spring", "stiffness": 0.3, "wind": 1 }] });
    let windy = json!({ "canvas": { "size": [200, 100], "wind": [300, 0] } });
    let mut rt = runtime(rigged(windy.clone(), json!([model("m", swing.clone())])));
    let rest = tip(&rt);
    rt.frame(VIEW, Quality::Q3);
    for _ in 0..10 {
        rt.advance(1.0 / 30.0);
        rt.frame(VIEW, Quality::Q3);
    }
    assert_ne!(tip(&rt), rest, "the wind pushes the tip");
    assert_eq!(rt.frame(VIEW, Quality::Q3).wake, Wake::Frame);
    let mut calm = runtime(rigged(json!({}), json!([model("m", swing.clone())])));
    for _ in 0..60 {
        calm.advance(1.0 / 30.0);
        calm.frame(VIEW, Quality::Q3);
    }
    assert_eq!(calm.frame(VIEW, Quality::Q3).wake, Wake::Idle, "a body at rest settles");
    let mut low = runtime(rigged(windy, json!([model("m", swing)])));
    low.set_quality(Quality::Q0);
    for _ in 0..10 {
        low.advance(1.0 / 30.0);
        low.frame(VIEW, Quality::Q0);
    }
    assert_eq!(tip(&low), rest, "frozen at the lowest quality");
}

#[test]
fn root_motion_needs_clips_and_names_model_animation() {
    let s = Scene::from_json(
        &json!({ "schema": SCHEMA, "canvas": { "size": [8, 8] }, "assets": { "toy": { "path": "toy.glb" } },
            "layers": [model("m", json!({ "root_motion": true }))] })
        .to_string(),
    )
    .unwrap();
    let issues = s.validate(&BTreeMap::new());
    assert!(issues.iter().any(|i| i.to_string().contains("root_motion")), "{issues:?}");
    assert!(s.features().contains("model_animation"));
}
