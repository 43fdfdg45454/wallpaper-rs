use super::*;

fn pooled(uses: &str) -> Runtime {
    runtime(json!({
        "pools": { "bars": { "type": "mesh", "max": 2, "max_vertices": 8, "size": [100, 50] } },
        "scripts": [{ "path": "bars.js", "uses": [uses] }],
        "layers": [{ "id": "bars", "type": "video", "video": "bars" }, { "id": "moon", "type": "solid", "color": "#ffffff" }]
    }))
}

fn mesh(generation: u64, vertices: usize, indices: Vec<u32>) -> Arc<Mesh> {
    Arc::new(Mesh { generation, shapes: 1, vertices: vec![1.0; vertices * MESH_FLOATS], indices })
}

#[test]
fn pools_validate_their_shapes_and_ask_for_no_frames_while_unchanged() {
    let mut rt = pooled("geometry");
    assert!(rt.scene.features().contains(&"script_geometry"));
    assert_eq!(rt.frame(VIEW, Quality::Q3).wake, Wake::Idle, "a layer that shows a pool is no video");
    let out = |meshes: Vec<(&str, Arc<Mesh>)>| ScriptOutputs {
        meshes: meshes.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(),
        ..ScriptOutputs::default()
    };
    assert!(rt.set_script(out(vec![("bars", mesh(1, 3, vec![0, 1, 2]))])));
    assert_eq!(rt.mesh("bars").map(|m| m.generation), Some(1));
    assert!(
        !rt.set_script(out(vec![("bars", mesh(1, 3, vec![0, 1, 2]))])),
        "the same generation changes nothing"
    );
    assert_eq!(rt.frame(VIEW, Quality::Q3).wake, Wake::Idle);
    for wrong in [mesh(2, 9, vec![0, 1, 2]), mesh(3, 3, vec![0, 1, 3]), mesh(4, 3, vec![0, 1])] {
        rt.set_script(out(vec![("bars", wrong)]));
        assert!(rt.mesh("bars").is_none(), "shapes past the limits of the pool are dropped");
    }
    let mut nan =
        Mesh { generation: 5, shapes: 1, vertices: vec![1.0; 3 * MESH_FLOATS], indices: vec![0, 1, 2] };
    nan.vertices[3] = f32::NAN;
    rt.set_script(out(vec![("bars", Arc::new(nan)), ("other", mesh(6, 3, vec![0, 1, 2]))]));
    assert!(rt.mesh("bars").is_none() && rt.mesh("other").is_none());
}

#[test]
fn parents_set_by_scripts_are_checked_for_loops_and_missing_layers() {
    let mut rt = pooled("hierarchy");
    let set = |parent: &str| LayerOverride { parent: Some(parent.into()), ..LayerOverride::default() };
    let layers = [("moon", set("bars")), ("bars", set("moon"))].map(|(k, v)| (k.to_owned(), v));
    rt.set_script(ScriptOutputs { layers: layers.into(), ..ScriptOutputs::default() });
    let parents: Vec<_> = ["moon", "bars"].iter().map(|id| rt.script().layers[*id].parent.clone()).collect();
    assert!(parents.iter().filter(|p| p.is_some()).count() <= 1, "{parents:?}");
    rt.set_script(ScriptOutputs {
        layers: [("moon".to_owned(), set("ghost"))].into(),
        ..ScriptOutputs::default()
    });
    assert_eq!(rt.script().layers["moon"].parent, None);
}
