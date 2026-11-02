use super::*;

fn split(glb: &[u8]) -> (Value, Vec<u8>) {
    let word = |at: usize| u32::from_le_bytes(glb[at..at + 4].try_into().unwrap()) as usize;
    let text = word(12);
    let doc = serde_json::from_slice(&glb[20..20 + text]).unwrap();
    (doc, glb[20 + text + 8..].to_vec())
}

fn pack(doc: &Value, bin: &[u8]) -> Vec<u8> {
    let mut text = doc.to_string().into_bytes();
    while !text.len().is_multiple_of(4) {
        text.push(b' ');
    }
    let total = 12 + 8 + text.len() + 8 + bin.len();
    let mut glb = Vec::with_capacity(total);
    for word in [0x4654_6C67, 2, total as u32, text.len() as u32, 0x4E4F_534A] {
        glb.extend_from_slice(&u32::to_le_bytes(word));
    }
    glb.extend_from_slice(&text);
    glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    glb.extend_from_slice(&[0x42, 0x49, 0x4E, 0x00]);
    glb.extend_from_slice(bin);
    glb
}

fn header(side: u32) -> Vec<u8> {
    let mut png = vec![0x89, b'P', b'N', b'G', 13, 10, 26, 10, 0, 0, 0, 13, b'I', b'H', b'D', b'R'];
    png.extend(side.to_be_bytes());
    png.extend(side.to_be_bytes());
    png.extend([8, 6, 0, 0, 0]);
    png
}

fn painted(side: u32) -> (Value, Vec<u8>) {
    let mut m = ForgeMaterial::new("paint", [1.0; 4]);
    m.maps[0] = Some(header(side));
    split(&Forge::default().material(m).part("p", ForgeShape::Plane, [0.0; 3], 1.0, 0).glb())
}

#[test]
fn textures_that_share_a_picture_hold_it_once_and_their_count_is_capped() {
    let (mut doc, bin) = painted(1024);
    doc["textures"] = json!(vec![json!({ "source": 0 }); 3000]);
    doc["materials"][0]["pbrMetallicRoughness"]["baseColorTexture"]["index"] = json!(2999);
    let m = Model::decode(&pack(&doc, &bin)).unwrap();
    assert_eq!((m.textures.len(), m.materials[0].maps[0]), (1, Some(0)));
    doc["textures"] = json!(vec![json!({ "source": 0 }); 5000]);
    assert!(Model::decode(&pack(&doc, &bin)).unwrap_err().contains("textures, at most 4096"));
}

#[test]
fn the_pixels_of_all_textures_of_a_model_are_capped() {
    let (mut doc, mut bin) = painted(4096);
    let (mut images, mut textures) = (Vec::new(), Vec::new());
    for i in 0..17 {
        let png = header(4096);
        let view = json!({ "buffer": 0, "byteOffset": bin.len(), "byteLength": png.len() });
        bin.extend(png);
        doc["bufferViews"].as_array_mut().unwrap().push(view);
        let at = doc["bufferViews"].as_array().unwrap().len() - 1;
        images.push(json!({ "bufferView": at, "mimeType": "image/png" }));
        textures.push(json!({ "source": i }));
    }
    (doc["images"], doc["textures"]) = (json!(images), json!(textures));
    doc["buffers"][0]["byteLength"] = json!(bin.len());
    assert!(Model::decode(&pack(&doc, &bin)).unwrap_err().contains("texture pixels"));
    doc["images"].as_array_mut().unwrap().truncate(16);
    doc["textures"].as_array_mut().unwrap().truncate(16);
    assert_eq!(Model::decode(&pack(&doc, &bin)).unwrap().textures.len(), 16);
}

#[test]
fn pictures_are_held_once_per_byte_range_and_overlapping_ranges_cannot_pass_the_file() {
    let (mut doc, mut bin) = painted(1);
    let view = doc["images"][0]["bufferView"].as_u64().unwrap() as usize;
    let at = doc["bufferViews"][view]["byteOffset"].as_u64().unwrap_or(0) as usize;
    bin.resize(bin.len() + (64 << 10), 0);
    doc["buffers"][0]["byteLength"] = json!(bin.len());
    let made = |doc: &mut Value, lengths: &dyn Fn(usize) -> usize| {
        let first = doc["bufferViews"].as_array().unwrap().len();
        for i in 0..4096 {
            let span = json!({ "buffer": 0, "byteOffset": at, "byteLength": lengths(i) });
            doc["bufferViews"].as_array_mut().unwrap().push(span);
        }
        doc["images"] = json!((0..4096).map(|i| json!({ "bufferView": first + i })).collect::<Vec<_>>());
        doc["textures"] = json!((0..4096).map(|i| json!({ "source": i })).collect::<Vec<_>>());
        pack(doc, &bin)
    };
    let whole = bin.len() - at;
    let same = made(&mut doc.clone(), &|_| whole);
    assert!(ModelFile::parse(&same).is_ok());
    assert_eq!(Model::decode(&same).unwrap().textures.len(), 1);
    let shifted = made(&mut doc, &|i| whole - i);
    assert!(ModelFile::parse(&shifted).unwrap_err().contains("overlapping pictures"));
    assert!(Model::decode(&shifted).unwrap_err().contains("overlapping pictures"));
    assert!(Model::decode_posed(&shifted).unwrap_err().contains("overlapping pictures"));
}

#[test]
fn every_node_that_places_a_mesh_counts_its_triangles() {
    let ball = Forge::default()
        .material(ForgeMaterial::new("m", [1.0; 4]))
        .part("b", ForgeShape::Sphere(130), [0.0; 3], 1.0, 0)
        .glb();
    let (mut doc, bin) = split(&ball);
    let one = ModelFile::parse(&ball).unwrap().triangles;
    let copies = (MAX_TRIANGLES / one + 1) as usize;
    doc["nodes"] = json!(vec![json!({ "mesh": 0 }); 3]);
    let three = pack(&doc, &bin);
    assert_eq!(ModelFile::parse(&three).unwrap().triangles, one * 3);
    assert_eq!(Model::decode(&three).unwrap().triangles, one * 3);
    doc["nodes"] = json!(vec![json!({ "mesh": 0 }); copies]);
    let many = pack(&doc, &bin);
    assert!(ModelFile::parse(&many).unwrap_err().contains("triangles"));
    assert!(Model::decode(&many).unwrap_err().contains("triangles"));
    assert!(Model::decode_posed(&many).unwrap_err().contains("triangles"));
}

#[test]
fn a_scene_refuses_models_that_together_pass_the_triangle_budget() {
    let ball = Forge::default()
        .material(ForgeMaterial::new("m", [1.0; 4]))
        .part("b", ForgeShape::Sphere(130), [0.0; 3], 1.0, 0)
        .glb();
    let (mut doc, bin) = split(&ball);
    doc["nodes"] = json!(vec![json!({ "mesh": 0 }); 20]);
    let heavy = pack(&doc, &bin);
    let assets = json!({ "a": { "path": "a.glb" }, "b": { "path": "b.glb" } });
    let layers =
        json!([{ "id": "x", "type": "model", "model": "a" }, { "id": "y", "type": "model", "model": "b" }]);
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [8, 8] }, "assets": assets, "layers": layers });
    let mut scene = Scene::from_json(&text.to_string()).unwrap();
    let read = scene.adopt(&BTreeMap::new(), |_: &str, _: usize| Ok::<_, String>(heavy.clone()));
    assert!(format!("{read:?}").contains("triangles the models of a scene may hold"), "{read:?}");
}

#[test]
fn a_tier_shows_the_poster_of_models_past_its_triangles_counting_fur_shells() {
    let ball = Forge::default()
        .material(ForgeMaterial::new("m", [1.0; 4]))
        .part("b", ForgeShape::Sphere(130), [0.0; 3], 1.0, 0)
        .glb();
    let made = |materials: Value| {
        let text = json!({ "schema": SCHEMA, "canvas": { "size": [8, 8] }, "assets": { "a": { "path": "a.glb" } },
            "layers": [{ "id": "x", "type": "model", "model": "a", "materials": materials }] });
        let mut scene = Scene::from_json(&text.to_string()).unwrap();
        scene.adopt(&BTreeMap::new(), |_: &str, _: usize| Ok::<_, String>(ball.clone())).unwrap();
        runtime(scene)
    };
    let plain = made(json!({}));
    let one = plain.scene.model_triangles(Quality::Q0);
    assert!(one > 50_000 && one < 75_000, "{one}");
    assert!(plain.cost(Quality::Q0) < Tier::T0.budget(), "the cost alone would let it play");
    assert!(Budget::new(Tier::T0, false, 60).still(&plain), "past the 50 000 triangles of T0");
    assert!(!Budget::new(Tier::T1, false, 60).still(&plain));
    let furry = made(json!({ "m": { "shader": "fur", "params": { "layers": 8 } } }));
    assert_eq!(furry.scene.model_triangles(Quality::Q0), one * 3);
    assert_eq!(furry.scene.model_triangles(Quality::Q3), one * 9);
    assert!(furry.cost(Quality::Q3) > furry.cost(Quality::Q0));
    assert!(Budget::new(Tier::T1, false, 60).still(&furry), "the shells count at the lowest quality");
    assert!(!Budget::new(Tier::T2, false, 60).still(&furry));
}

#[test]
fn texture_features_models_skip_are_refused_when_required_and_noted_otherwise() {
    let (mut doc, bin) = painted(16);
    assert!(ModelFile::parse(&pack(&doc, &bin)).unwrap().notes.is_empty());
    doc["extensionsUsed"] = json!(["KHR_texture_transform"]);
    doc["materials"][0]["pbrMetallicRoughness"]["baseColorTexture"]["texCoord"] = json!(1);
    doc["samplers"] = json!([{ "wrapS": 33071 }]);
    let noted = pack(&doc, &bin);
    let notes = ModelFile::parse(&noted).unwrap().notes;
    assert_eq!(notes.len(), 3, "{notes:?}");
    assert!(Model::decode(&noted).is_ok());
    doc["extensionsRequired"] = json!(["KHR_texture_transform"]);
    let required = pack(&doc, &bin);
    assert!(ModelFile::parse(&required).is_ok(), "mesh layers keep reading it");
    assert!(Model::decode(&required).unwrap_err().contains("KHR_texture_transform"));
}
