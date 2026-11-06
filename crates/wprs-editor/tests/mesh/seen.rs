use super::kit::{act, info, query, rigged};
use serde_json::{Value, json};
use wprs_editor::Project;

fn twinned(p: &mut Project) {
    let path = p.doc["scene"]["assets"]["net"]["path"].as_str().unwrap().to_owned();
    let bytes = std::fs::read(p.dir.join(&path)).unwrap();
    let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
    let text = word(12);
    let mut doc: Value = serde_json::from_slice(&bytes[20..20 + text]).unwrap();
    let bin = bytes[28 + text..28 + text + word(20 + text)].to_vec();
    let drawn = doc["nodes"].as_array().unwrap().iter().find(|n| n.get("mesh").is_some()).unwrap().clone();
    let mut twin = json!({ "name": "twin", "mesh": drawn["mesh"], "translation": [400, 0, 0] });
    if let Some(s) = drawn.get("skin") {
        twin["skin"] = s.clone();
    }
    let nodes = doc["nodes"].as_array_mut().unwrap();
    nodes.push(twin);
    let at = nodes.len() - 1;
    doc["scenes"][0]["nodes"].as_array_mut().unwrap().push(json!(at));
    let mut text = doc.to_string().into_bytes();
    text.resize(text.len().div_ceil(4) * 4, b' ');
    let mut out = Vec::new();
    for w in [0x4654_6C67u32, 2, (28 + text.len() + bin.len()) as u32, text.len() as u32, 0x4E4F_534A] {
        out.extend(w.to_le_bytes());
    }
    out.extend(text);
    out.extend((bin.len() as u32).to_le_bytes());
    out.extend(0x004E_4942u32.to_le_bytes());
    out.extend(bin);
    std::fs::write(p.dir.join("meshes/twin.glb"), out).unwrap();
    act(p, json!({ "set": { "pointer": "/scene/assets/net/path", "value": "meshes/twin.glb" } })).unwrap();
}

#[test]
fn a_mesh_drawn_twice_gets_a_mesh_of_its_own_for_each_drawing_once_it_is_edited() {
    let root = tempfile::tempdir().unwrap();
    let mut p = rigged(&root);
    let before = info(&p, "net");
    let outline = query(&p, "net", json!({ "outline": true }));
    twinned(&mut p);
    let seen = info(&p, "net");
    assert_eq!(seen["editable"], json!(true), "a mesh drawn twice is edited like any other");
    let at = |v: &Value, name: &str| {
        v["bones"].as_array().unwrap().iter().find(|b| b["name"] == name).unwrap()["at"].clone()
    };
    for bone in ["root", "arm"] {
        assert!(at(&seen, bone).is_array(), "{seen}");
        assert_eq!(at(&seen, bone), at(&before, bone));
    }
    let rims = query(&p, "net", json!({ "outline": true }));
    let (old, new) = (outline.as_object().unwrap(), rims.as_object().unwrap());
    assert!(!new.is_empty(), "{rims}");
    for (part, edges) in old {
        let drawn = new[part].as_array().unwrap();
        assert!(drawn.len() >= 2 * edges.as_array().unwrap().len(), "both copies are outlined: {part}");
    }
    act(&mut p, json!({ "ik": { "layer": "net", "root": "root", "tip": "arm", "target": [10, 10] } }))
        .unwrap();
    let handles = query(&p, "net", json!({ "handles": true }));
    let chain = &handles["ik"][0];
    assert!(chain["at"].as_array().is_some_and(|a| a.len() == 2 && a[0].is_array()), "{handles}");
    act(&mut p, json!({ "bone": { "layer": "net", "name": "hand", "parent": "arm", "at": [0.8, 0.5] } }))
        .unwrap();
    let path = p.doc["scene"]["assets"]["net"]["path"].as_str().unwrap().to_owned();
    let rig = wprs_scene::Rigged::decode(&std::fs::read(p.dir.join(path)).unwrap()).unwrap();
    assert!(rig.joint("hand").is_some() && rig.parts.len() >= 2, "{:?}", rig.parts);
    let sources: Vec<[u16; 2]> = rig.pieces.iter().map(|piece| piece.source).collect();
    assert!(sources.windows(2).all(|w| w[0] != w[1]), "{sources:?}");
}
