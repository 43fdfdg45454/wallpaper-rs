use super::*;

#[test]
fn the_schema_describes_every_advanced_field() {
    let schema = schema().to_string();
    let words = [
        "mesh",
        "model",
        "clips",
        "animations",
        "ik",
        "physics",
        "rules",
        "shapes",
        "expressions",
        "parts",
        "hit",
        "materials",
        "root_motion",
        "camera3d",
        "paths",
        "fade",
        "attach",
        "depth_test",
        "space",
        "3d",
        "wind",
        "shadows",
        "children",
        "points",
        "lock",
        "periodic",
        "max_per_step",
        "audio_gate",
        "palette",
        "hsv",
        "velocity",
        "turbulent",
        "offset_noise",
        "remap",
        "window",
        "oscillate",
        "boids",
        "keep_between",
        "rope",
        "cutout",
        "refract",
        "instance",
        "templates",
        "pools",
        "events",
        "uses",
        "fn",
        "media",
        "rgb",
        "timelineEvent",
        "mediaThumbnail",
        "storage",
        "album_artist",
        "previous",
    ];
    for word in words {
        assert!(schema.contains(&format!("\"{word}\"")), "the scene schema lacks {word}");
    }
}

#[test]
fn each_model_file_is_read_once_and_its_counts_cannot_overflow() {
    let mut doc: Json = serde_json::from_str(DEEP).unwrap();
    doc["assets"]["robot"]["path"] = json!("models/fox.glb");
    let mut scene = Scene::from_json(&doc.to_string()).unwrap();
    let (model, mut reads) = (figure(), 0);
    let mut inner = files(&model);
    let counted = |path: &str, max: usize| {
        reads += usize::from(path.ends_with(".glb"));
        inner(path, max)
    };
    scene.adopt(&BTreeMap::new(), counted).unwrap();
    assert_eq!(reads, 2);
    assert!(std::sync::Arc::ptr_eq(&scene.models["fox"], &scene.models["robot"]));
    let mut huge = figure();
    huge["accessors"][3]["count"] = json!(u64::MAX);
    huge["meshes"][1]["primitives"] =
        json!(vec![json!({ "attributes": { "POSITION": 3 }, "indices": 3 }); 4]);
    assert!(ModelFile::parse(&glb(&huge)).unwrap_err().contains("triangles"));
    let mut wide = glb(&figure());
    wide[12..16].copy_from_slice(&(5u32 << 20).to_le_bytes());
    assert!(ModelFile::parse(&wide).unwrap_err().contains("more than 4 MiB of JSON"));
    let mut empty = figure();
    empty["nodes"] = json!(vec![json!({}); wprs_scene::MAX_NODES + 1]);
    assert!(ModelFile::parse(&glb(&empty)).unwrap_err().contains("nodes, at most"));
    let mut crowded = figure();
    crowded["accessors"] = json!(vec![json!({}); wprs_scene::MAX_MODEL_ENTRIES]);
    assert!(ModelFile::parse(&glb(&crowded)).unwrap_err().contains("more entries than"));
    let mut left = wprs_scene::MAX_MODEL_ENTRIES;
    let (bytes, half) = (glb(&figure()), wprs_scene::MAX_MODEL_ENTRIES / 2);
    let mut halved = figure();
    halved["accessors"] = json!(vec![json!({}); half]);
    let halved = glb(&halved);
    ModelFile::parse_head(&halved, halved.len() as u64, &mut left).unwrap();
    ModelFile::parse_head(&bytes, bytes.len() as u64, &mut left).unwrap();
    assert!(
        ModelFile::parse_head(&halved, halved.len() as u64, &mut left).unwrap_err().contains("more entries")
    );
}
